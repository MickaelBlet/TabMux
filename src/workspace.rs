//! Workspace files: TOML description of tabs, split layouts and pane commands.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::layout::{Dir, MAX_RATIO, MIN_RATIO, Node, PaneId};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Workspace {
    /// Index of the initially active tab.
    pub active: Option<usize>,
    /// Show pane title rows at startup (default: true).
    pub titles: Option<bool>,
    #[serde(default)]
    pub tab: Vec<TabSpec>,
    /// Shell profiles offered by the new-tab menu.
    #[serde(default)]
    pub profile: Vec<ProfileSpec>,
    /// Directory of the workspace file, used to resolve relative paths.
    #[serde(skip)]
    pub base_dir: PathBuf,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TabSpec {
    pub name: Option<String>,
    pub cwd: Option<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    pub layout: Option<NodeSpec>,
}

/// A named way to start a shell, picked from the `[⌵]` menu.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileSpec {
    pub name: String,
    /// Program and arguments; replaces `$SHELL`.
    pub shell: Option<Vec<String>>,
    pub cwd: Option<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    pub command: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SplitDir {
    Vertical,
    Horizontal,
}

impl From<SplitDir> for Dir {
    fn from(d: SplitDir) -> Self {
        match d {
            SplitDir::Vertical => Dir::Vertical,
            SplitDir::Horizontal => Dir::Horizontal,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum NodeSpec {
    Split(SplitSpec),
    Pane(PaneSpec),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SplitSpec {
    pub split: SplitDir,
    pub panes: Vec<NodeSpec>,
    pub size: Option<f32>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaneSpec {
    pub cwd: Option<String>,
    pub command: Option<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub focus: bool,
    pub size: Option<f32>,
}

impl NodeSpec {
    fn size(&self) -> f32 {
        match self {
            NodeSpec::Split(s) => s.size,
            NodeSpec::Pane(p) => p.size,
        }
        .unwrap_or(1.0)
    }

    fn validate(&self, at: &str) -> Result<()> {
        let size = self.size();
        if !(size.is_finite() && size > 0.0) {
            bail!("{at}: size must be > 0");
        }
        if let NodeSpec::Split(s) = self {
            if s.panes.is_empty() {
                bail!("{at}: split has no panes");
            }
            for (i, c) in s.panes.iter().enumerate() {
                c.validate(&format!("{at}.panes[{i}]"))?;
            }
        }
        Ok(())
    }

    /// Build a layout tree, calling `spawn` for each pane in order.
    /// Returns the tree and the pane marked `focus`, if any.
    pub fn build<F>(&self, spawn: &mut F) -> Result<(Node, Option<PaneId>)>
    where
        F: FnMut(&PaneSpec) -> Result<PaneId>,
    {
        match self {
            NodeSpec::Pane(p) => {
                let id = spawn(p)?;
                Ok((Node::Leaf(id), p.focus.then_some(id)))
            }
            NodeSpec::Split(s) => build_split(s.split.into(), &s.panes, spawn),
        }
    }
}

/// Fold N children into nested binary splits, preserving relative sizes.
fn build_split<F>(dir: Dir, children: &[NodeSpec], spawn: &mut F) -> Result<(Node, Option<PaneId>)>
where
    F: FnMut(&PaneSpec) -> Result<PaneId>,
{
    let (first, rest) = children.split_first().context("empty split")?;
    let (a, fa) = first.build(spawn)?;
    if rest.is_empty() {
        return Ok((a, fa));
    }
    let total: f32 = children.iter().map(NodeSpec::size).sum();
    let ratio = (first.size() / total).clamp(MIN_RATIO, MAX_RATIO);
    let (b, fb) = build_split(dir, rest, spawn)?;
    let node = Node::Split {
        dir,
        ratio,
        a: Box::new(a),
        b: Box::new(b),
    };
    Ok((node, fa.or(fb)))
}

impl Workspace {
    pub fn load(path: &Path) -> Result<Self> {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        let mut ws = Self::parse(&text).with_context(|| format!("parse {}", path.display()))?;
        ws.base_dir = path
            .canonicalize()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf))
            .unwrap_or_default();
        Ok(ws)
    }

    pub fn parse(text: &str) -> Result<Self> {
        let ws: Self = toml::from_str(text)?;
        if ws.tab.is_empty() {
            bail!("workspace has no [[tab]]");
        }
        for (i, t) in ws.tab.iter().enumerate() {
            if let Some(l) = &t.layout {
                l.validate(&format!("tab[{i}].layout"))?;
            }
        }
        let mut names = std::collections::HashSet::new();
        for (i, p) in ws.profile.iter().enumerate() {
            if p.name.trim().is_empty() {
                bail!("profile[{i}]: name is empty");
            }
            if !names.insert(p.name.as_str()) {
                bail!("profile[{i}]: duplicate name {:?}", p.name);
            }
            if p.shell.as_ref().is_some_and(|s| s.is_empty()) {
                bail!("profile[{i}]: shell is empty");
            }
        }
        Ok(ws)
    }

    /// Resolve a pane's cwd: pane overrides tab; `~` expands to `$HOME`,
    /// relative paths are relative to the workspace file.
    pub fn resolve_cwd(&self, tab: Option<&str>, pane: Option<&str>) -> Option<PathBuf> {
        let raw = pane.or(tab)?;
        let path = match raw.strip_prefix('~') {
            Some(rest) if rest.is_empty() || rest.starts_with('/') => {
                let home = std::env::var_os("HOME")?;
                PathBuf::from(home).join(rest.trim_start_matches('/'))
            }
            _ => PathBuf::from(raw),
        };
        Some(if path.is_relative() {
            self.base_dir.join(path)
        } else {
            path
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = r#"
active = 1
titles = false

[[tab]]
name = "dev"
cwd = "~/proj"
env = { RUST_LOG = "debug" }

[tab.layout]
split = "vertical"
[[tab.layout.panes]]
size = 3
command = "nvim ."
[[tab.layout.panes]]
split = "horizontal"
panes = [{ command = "cargo watch" }, { cwd = "docs", focus = true }]

[[tab]]
name = "logs"
"#;

    #[test]
    fn parse_example() {
        let ws = Workspace::parse(EXAMPLE).unwrap();
        assert_eq!(ws.active, Some(1));
        assert_eq!(ws.titles, Some(false));
        assert_eq!(ws.tab.len(), 2);
        assert_eq!(ws.tab[0].env["RUST_LOG"], "debug");
        assert!(ws.tab[1].layout.is_none());
        let mut next = 0;
        let (node, focus) = ws.tab[0]
            .layout
            .as_ref()
            .unwrap()
            .build(&mut |_| {
                next += 1;
                Ok(next - 1)
            })
            .unwrap();
        assert_eq!(node.leaves(), vec![0, 1, 2]);
        assert_eq!(focus, Some(2));
        match node {
            Node::Split { dir, ratio, .. } => {
                assert_eq!(dir, Dir::Vertical);
                assert_eq!(ratio, 0.75);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn nary_fold_equal() {
        let ws = Workspace::parse(
            "[[tab]]\n[tab.layout]\nsplit = \"horizontal\"\npanes = [{}, {}, {}]\n",
        )
        .unwrap();
        let mut next = 0;
        let (node, focus) = ws.tab[0]
            .layout
            .as_ref()
            .unwrap()
            .build(&mut |_| {
                next += 1;
                Ok(next - 1)
            })
            .unwrap();
        assert!(focus.is_none());
        let Node::Split { ratio, b, .. } = node else {
            panic!()
        };
        assert!((ratio - 1.0 / 3.0).abs() < 1e-6);
        let Node::Split { ratio, .. } = *b else {
            panic!()
        };
        assert_eq!(ratio, 0.5);
    }

    #[test]
    fn errors() {
        assert!(Workspace::parse("").is_err());
        assert!(
            Workspace::parse("[[tab]]\nlayout = { split = \"vertical\", panes = [] }").is_err()
        );
        assert!(Workspace::parse("[[tab]]\nbogus = 1").is_err());
        assert!(Workspace::parse("[[tab]]\nlayout = { size = 0 }").is_err());
    }

    #[test]
    fn profiles() {
        let ws = Workspace::parse(
            "[[tab]]\n[[profile]]\nname = \"fish\"\nshell = [\"fish\", \"-l\"]\nenv = { A = \"1\" }\n\
             [[profile]]\nname = \"top\"\ncommand = \"htop\"\n",
        )
        .unwrap();
        assert_eq!(ws.profile.len(), 2);
        assert_eq!(ws.profile[0].shell.as_deref().unwrap(), ["fish", "-l"]);
        assert_eq!(ws.profile[0].env["A"], "1");
        assert_eq!(ws.profile[1].command.as_deref(), Some("htop"));
        for bad in [
            "[[tab]]\n[[profile]]\nname = \"\"",
            "[[tab]]\n[[profile]]\nname = \"a\"\n[[profile]]\nname = \"a\"",
            "[[tab]]\n[[profile]]\nname = \"a\"\nshell = []",
            "[[tab]]\n[[profile]]\nname = \"a\"\nbogus = 1",
            "[[tab]]\n[[profile]]\nshell = [\"sh\"]",
        ] {
            assert!(Workspace::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn examples_parse() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
        let mut n = 0;
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "toml") {
                Workspace::load(&path).unwrap();
                n += 1;
            }
        }
        assert!(n > 0);
    }

    #[test]
    fn cwd_resolution() {
        let ws = Workspace {
            active: None,
            titles: None,
            tab: vec![],
            profile: vec![],
            base_dir: PathBuf::from("/ws"),
        };
        assert_eq!(ws.resolve_cwd(None, None), None);
        assert_eq!(
            ws.resolve_cwd(Some("/a"), Some("b")),
            Some(PathBuf::from("/ws/b"))
        );
        assert_eq!(ws.resolve_cwd(Some("/a"), None), Some(PathBuf::from("/a")));
        let home = PathBuf::from(std::env::var_os("HOME").unwrap());
        assert_eq!(ws.resolve_cwd(Some("~/x"), None), Some(home.join("x")));
    }
}
