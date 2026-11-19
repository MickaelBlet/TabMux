#!/bin/sh
# Regenerate docs/img/screen.svg from a real TabMux session.
# Runs tabmux inside a detached tmux with a neutral HOME/prompt, captures the
# screen with colors and renders it to SVG. Requires tmux and python3.
set -eu

ROOT=$(cd "$(dirname "$0")/.." && pwd)
COLS=120
ROWS=28
WORK=$(mktemp -d)
trap 'tmux kill-session -t tabmux-shot 2>/dev/null || true; rm -rf "$WORK"' EXIT

cargo build --release --manifest-path "$ROOT/Cargo.toml" >/dev/null

# Neutral home: no personal paths or prompt in the picture.
H="$WORK/home"
mkdir -p "$H/tabmux"
cp -r "$ROOT/src" "$ROOT/Cargo.toml" "$ROOT/.git" "$H/tabmux/"
touch "$H/.sudo_as_admin_successful"
cat >"$H/.bashrc" <<'EOF'
PS1='\[\e[32m\]user@host\[\e[0m\]:\[\e[34m\]\w\[\e[0m\]$ '
PROMPT_COMMAND='printf "\e]0;%s\a" "${PWD/#$HOME/\~}"'
EOF

cat >"$WORK/demo.toml" <<EOF
[[tab]]
name = "code"
cwd = "$H/tabmux"
[tab.layout]
split = "vertical"
[[tab.layout.panes]]
size = 2
command = "ls src"
focus = true
[[tab.layout.panes]]
split = "horizontal"
panes = [
  { command = "git log --oneline -5" },
  { command = "wc -l src/*.rs | sort -n | tail -6" },
]

[[tab]]
name = "git"
cwd = "$H/tabmux"

[[profile]]
name = "sh"
shell = ["/bin/sh"]
EOF

cat >"$WORK/run.sh" <<EOF
#!/bin/sh
exec env -i PATH=/usr/local/bin:/usr/bin:/bin HOME="$H" USER=user SHELL=/bin/bash \
  TERM=xterm-256color LANG=C.UTF-8 "$ROOT/target/release/tabmux" -w "$WORK/demo.toml"
EOF
chmod +x "$WORK/run.sh"

tmux new-session -d -s tabmux-shot -x "$COLS" -y "$ROWS" "$WORK/run.sh"
sleep 4
mkdir -p "$ROOT/docs/img"
tmux capture-pane -t tabmux-shot -e -p |
  python3 "$ROOT/scripts/ansi2svg.py" "$COLS" "$ROWS" >"$ROOT/docs/img/screen.svg"
echo "Wrote docs/img/screen.svg"
