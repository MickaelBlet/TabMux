# Shell integration

TabMux uses **prompt marks** to [jump between commands](features.md#prompt-navigation).
A shell tells the terminal where a prompt starts by printing the
**OSC 133;A** escape sequence (`ESC ] 133 ; A BEL`) just before the prompt.
This is the FinalTerm convention, also used by VS Code, iTerm2, WezTerm and
others.

## Automatic setup (zsh, bash)

When a pane's shell is `zsh` or `bash` (matched by the program's file name),
TabMux sets up the integration itself. It **never changes your config
files**.

At spawn time TabMux writes small scripts, embedded in the binary from
`src/shell/`, to a per-user directory:

```text
$TMPDIR/tabmux-$USER/
├── bashrc
└── zsh/
    ├── .zshenv
    └── .zshrc
```

A file is only rewritten when its content has changed.

### zsh

1. TabMux saves your `ZDOTDIR` (empty if unset) in `TABMUX_ZDOTDIR` and
   points `ZDOTDIR` at `$TMPDIR/tabmux-$USER/zsh`.
2. The TabMux `.zshenv` sources your own `$ZDOTDIR/.zshenv` (or
   `~/.zshenv`). If that file changes `ZDOTDIR`, the new value is kept.
3. The TabMux `.zshrc` puts `ZDOTDIR` back to your value, sources your
   `.zshrc`, then adds a `precmd` hook that prints OSC 133;A.

`.zprofile`, `.zlogin` and the rest load from your real `ZDOTDIR` as usual.

### bash

TabMux starts bash with `--rcfile $TMPDIR/tabmux-$USER/bashrc`. That file
sources `~/.bashrc`, then adds a function to `PROMPT_COMMAND` that prints
OSC 133;A. It handles both the string form and the array form (bash 5.1+) of
`PROMPT_COMMAND`.

`--rcfile` only applies to interactive non-login shells. A profile such as
`shell = ["bash", "-l"]` starts a login shell, which ignores `--rcfile`.

## Disabling

```sh
TABMUX_NO_SHELL_INTEGRATION=1 tabmux
```

If this variable is set to any value, TabMux leaves zsh and bash alone. The
integration is also skipped quietly when the temp directory can't be written.

## Other shells (manual setup)

TabMux picks up OSC 133;A from any program. For other shells, print it before
each prompt. You can check `$TABMUX` to do this only inside TabMux.

**fish** (`~/.config/fish/config.fish`):

```fish
if set -q TABMUX
    function __tabmux_prompt_mark --on-event fish_prompt
        printf '\e]133;A\a'
    end
end
```

**zsh / bash by hand** (if you disabled the automatic setup):

```sh
# zsh
precmd() { print -n '\e]133;A\a' }
# bash
PROMPT_COMMAND="${PROMPT_COMMAND:+$PROMPT_COMMAND;}printf '\e]133;A\a'"
```

Prompt frameworks that already emit OSC 133 (starship, oh-my-posh, …) work
with no extra setup.

## How marks are stored

- A mark is only recorded on the normal screen. Full-screen programs don't
  create marks.
- Marks are stored as absolute line numbers. When the 100 000-line scrollback
  is full and old lines are dropped, TabMux counts them so the marks stay
  lined up with the text.
- A new prompt at or above an older mark (after `clear`, for example)
  removes the marks it covers.

See [Architecture](architecture.md#prompt-marks) for implementation details.
