//! Automatic shell integration: make zsh and bash emit prompt marks (OSC 133;A)
//! without touching the user's config. Disable with `TABMUX_NO_SHELL_INTEGRATION=1`.

use std::fs;
use std::path::{Path, PathBuf};

use portable_pty::CommandBuilder;

const ZSHENV: &str = include_str!("shell/zshenv");
const ZSHRC: &str = include_str!("shell/zshrc");
const BASHRC: &str = include_str!("shell/bashrc");

/// Adjust `cmd` (which runs `shell`) to load the integration script.
pub fn inject(cmd: &mut CommandBuilder, shell: &str) {
    if std::env::var_os("TABMUX_NO_SHELL_INTEGRATION").is_some() {
        return;
    }
    let name = Path::new(shell)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let Some(dir) = write_scripts() else {
        return;
    };
    match name {
        "zsh" => {
            let user = std::env::var("ZDOTDIR").unwrap_or_default();
            cmd.env("TABMUX_ZDOTDIR", user);
            cmd.env("ZDOTDIR", dir.join("zsh"));
        }
        "bash" => {
            cmd.arg("--rcfile");
            cmd.arg(dir.join("bashrc"));
        }
        _ => {}
    }
}

/// Write the scripts to a per-user temp dir; `None` on failure.
fn write_scripts() -> Option<PathBuf> {
    let user = std::env::var("USER").unwrap_or_else(|_| "user".into());
    let dir = std::env::temp_dir().join(format!("tabmux-{user}"));
    fs::create_dir_all(dir.join("zsh")).ok()?;
    for (path, content) in [
        ("zsh/.zshenv", ZSHENV),
        ("zsh/.zshrc", ZSHRC),
        ("bashrc", BASHRC),
    ] {
        let p = dir.join(path);
        if fs::read_to_string(&p).ok().as_deref() != Some(content) {
            fs::write(&p, content).ok()?;
        }
    }
    Some(dir)
}
