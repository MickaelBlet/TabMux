#!/bin/sh
# Regenerate the docs/img/*.svg captures from real TabMux sessions.
# Runs tabmux inside a detached tmux with a neutral HOME/prompt, captures the
# screen with colors and renders it to SVG (buttons as Nerd Font pills, caps
# drawn as shapes). Requires tmux and python3.
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
  TERM=xterm-256color LANG=C.UTF-8 TABMUX_NERD_FONT=1 "$ROOT/target/release/tabmux" -w "$WORK/demo.toml"
EOF
chmod +x "$WORK/run.sh"

mkdir -p "$ROOT/docs/img"

# shot NAME [KEYS...]: start a fresh session, send each tmux key argument
# (a pause follows each one), then capture to docs/img/NAME.svg.
shot() {
  name=$1
  shift
  tmux kill-session -t tabmux-shot 2>/dev/null || true
  tmux new-session -d -s tabmux-shot -x "$COLS" -y "$ROWS" "$WORK/run.sh"
  sleep 3
  for k in "$@"; do
    tmux send-keys -t tabmux-shot "$k"
    sleep 0.5
  done
  sleep 1
  tmux capture-pane -t tabmux-shot -e -p |
    python3 "$ROOT/scripts/ansi2svg.py" "$COLS" "$ROWS" >"$ROOT/docs/img/$name.svg"
  echo "Wrote docs/img/$name.svg"
}

shot screen
shot topbar C-b t
shot help C-b '?'
shot search C-b / r s
shot multicursor C-b m 'echo hello' Enter
