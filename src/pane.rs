use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};

use crate::event::AppEvent;
use crate::marks::Marks;
use crate::shell;

const SCROLLBACK_LEN: usize = 100_000;
/// Output arriving this soon after a resize is a redraw, not activity.
const RESIZE_GRACE: Duration = Duration::from_millis(300);
/// Extra quiet time after spawn, on top of RESIZE_GRACE.
const STARTUP_GRACE: Duration = Duration::from_millis(700);
/// PTY width used by wide mode.
pub const WIDE_COLS: u16 = 1000;

/// Captures escape sequences that don't affect the screen (window title, prompt marks).
#[derive(Default)]
pub struct Callbacks {
    title: String,
    pub marks: Marks,
    /// Set on BEL / visual bell, cleared by `Pane::take_bell`.
    bell: bool,
    /// Set when the PTY produces output, cleared by `Pane::take_output`.
    output: bool,
    resized_at: Option<Instant>,
}

impl vt100::Callbacks for Callbacks {
    fn audible_bell(&mut self, _: &mut vt100::Screen) {
        self.bell = true;
    }

    fn visual_bell(&mut self, _: &mut vt100::Screen) {
        self.bell = true;
    }

    fn set_window_title(&mut self, _: &mut vt100::Screen, title: &[u8]) {
        self.title = String::from_utf8_lossy(title).into_owned();
    }

    fn unhandled_osc(&mut self, screen: &mut vt100::Screen, params: &[&[u8]]) {
        // OSC 133;A: prompt start (FinalTerm / VS Code shell integration).
        let prompt = params.first() == Some(&&b"133"[..])
            && params.get(1).is_some_and(|p| p.first() == Some(&b'A'));
        if prompt && !screen.alternate_screen() {
            let row = screen.cursor_position().0 as usize;
            self.marks.add(scrollback_len(screen) + row);
        }
    }
}

/// Number of lines currently held in the scrollback.
fn scrollback_len(screen: &mut vt100::Screen) -> usize {
    let cur = screen.scrollback();
    screen.set_scrollback(usize::MAX);
    let len = screen.scrollback();
    screen.set_scrollback(cur);
    len
}

/// Feed PTY output, counting lines evicted from a full scrollback so prompt
/// marks stay aligned. Only line feeds on the last row are counted.
fn feed(parser: &mut Parser, mut data: &[u8]) {
    while let Some(i) = data.iter().position(|&b| b == b'\n') {
        parser.process(&data[..i]);
        let screen = parser.screen_mut();
        let evicts = !screen.alternate_screen()
            && screen.cursor_position().0 + 1 == screen.size().0
            && scrollback_len(screen) == SCROLLBACK_LEN;
        parser.process(b"\n");
        if evicts {
            parser.callbacks_mut().marks.evict();
        }
        data = &data[i + 1..];
    }
    parser.process(data);
}

pub type Parser = vt100::Parser<Callbacks>;

/// Per-pane spawn options (from a workspace file).
#[derive(Default, Clone, Debug)]
pub struct SpawnOpts {
    pub cwd: Option<PathBuf>,
    /// Typed into the shell right after spawn; the shell stays alive after it exits.
    pub command: Option<String>,
    pub env: Vec<(String, String)>,
    /// Program and arguments to run instead of `$SHELL`.
    pub shell: Option<Vec<String>>,
}

pub struct Pane {
    pub id: usize,
    /// Wide mode: the PTY is WIDE_COLS wide and viewed through a horizontal window.
    pub wide: bool,
    /// First visible column in wide mode.
    pub hscroll: u16,
    /// Keep the cursor visible horizontally (set on typing, cleared by manual scroll).
    pub follow: bool,
    /// Last prompt jumped to: (history index, scroll offset set for it).
    pub jump: Option<(usize, usize)>,
    pub parser: Arc<Mutex<Parser>>,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
}

impl Pane {
    pub fn spawn(
        id: usize,
        rows: u16,
        cols: u16,
        tx: Sender<AppEvent>,
        opts: &SpawnOpts,
    ) -> Result<Self> {
        let (rows, cols) = (rows.max(1), cols.max(1));
        let pair = native_pty_system()
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("openpty")?;

        let (shell, args) = match opts.shell.as_deref() {
            Some([prog, args @ ..]) => (prog.clone(), args),
            _ => (
                std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".into()),
                &[][..],
            ),
        };
        let mut cmd = CommandBuilder::new(&shell);
        // Before user args: bash wants long options (--rcfile) first.
        shell::inject(&mut cmd, &shell);
        cmd.args(args);
        if let Some(cwd) = opts.cwd.clone().or_else(|| std::env::current_dir().ok()) {
            cmd.cwd(cwd);
        }
        cmd.env("TERM", "xterm-256color");
        cmd.env("TABMUX", "1");
        for (k, v) in &opts.env {
            cmd.env(k, v);
        }

        let child = pair.slave.spawn_command(cmd).context("spawn shell")?;
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader()?;
        let mut writer = pair.master.take_writer()?;
        if let Some(c) = &opts.command {
            writer.write_all(format!("{c}\n").as_bytes())?;
            writer.flush()?;
        }
        let parser = Arc::new(Mutex::new(Parser::new_with_callbacks(
            rows,
            cols,
            SCROLLBACK_LEN,
            Callbacks {
                // Treat the shell's startup prompt like a resize redraw.
                resized_at: Some(Instant::now() + STARTUP_GRACE),
                ..Default::default()
            },
        )));

        let p = Arc::clone(&parser);
        thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let mut parser = p.lock().unwrap();
                        feed(&mut parser, &buf[..n]);
                        let cb = parser.callbacks_mut();
                        if cb.resized_at.is_none_or(|t| t.elapsed() > RESIZE_GRACE) {
                            cb.output = true;
                        }
                        drop(parser);
                        if tx.send(AppEvent::PtyOutput).is_err() {
                            return;
                        }
                    }
                }
            }
            let _ = tx.send(AppEvent::PtyExit(id));
        });

        Ok(Self {
            id,
            wide: false,
            hscroll: 0,
            follow: false,
            jump: None,
            parser,
            master: pair.master,
            writer,
            child,
        })
    }

    pub fn title(&self) -> String {
        let parser = self.parser.lock().unwrap();
        let t = parser.callbacks().title.trim();
        if t.is_empty() {
            format!("shell {}", self.id + 1)
        } else {
            t.to_string()
        }
    }

    /// Returns and clears the pending bell flag.
    pub fn take_bell(&self) -> bool {
        std::mem::take(&mut self.parser.lock().unwrap().callbacks_mut().bell)
    }

    /// Returns and clears the pending output flag.
    pub fn take_output(&self) -> bool {
        std::mem::take(&mut self.parser.lock().unwrap().callbacks_mut().output)
    }

    pub fn write(&mut self, bytes: &[u8]) {
        let _ = self.writer.write_all(bytes);
        let _ = self.writer.flush();
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        let (rows, cols) = (rows.max(1), cols.max(1));
        let mut parser = self.parser.lock().unwrap();
        if parser.screen().size() == (rows, cols) {
            return;
        }
        parser.screen_mut().set_size(rows, cols);
        parser.callbacks_mut().resized_at = Some(Instant::now());
        let _ = self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        });
    }

    pub fn scroll(&mut self, delta: isize) {
        let mut parser = self.parser.lock().unwrap();
        let cur = parser.screen().scrollback() as isize;
        parser
            .screen_mut()
            .set_scrollback((cur + delta).max(0) as usize);
    }

    /// Current scrollback offset and maximum available offset.
    pub fn scroll_info(&self) -> (usize, usize) {
        let mut parser = self.parser.lock().unwrap();
        let cur = parser.screen().scrollback();
        parser.screen_mut().set_scrollback(usize::MAX);
        let max = parser.screen().scrollback();
        parser.screen_mut().set_scrollback(cur);
        (cur, max)
    }

    /// Every history line (scrollback then screen) and the max scrollback offset.
    /// Line `i` is visible at scroll offset `o` on row `i - (max - o)`.
    pub fn history(&self) -> (Vec<String>, usize) {
        let mut parser = self.parser.lock().unwrap();
        let screen = parser.screen_mut();
        let cur = screen.scrollback();
        screen.set_scrollback(usize::MAX);
        let max = screen.scrollback();
        let (rows, cols) = screen.size();
        let rows = rows as usize;
        let mut lines = vec![String::new(); max + rows];
        let mut offset = max;
        loop {
            screen.set_scrollback(offset);
            let top = max - offset;
            for (r, text) in screen.rows(0, cols).enumerate() {
                if let Some(l) = lines.get_mut(top + r) {
                    *l = text;
                }
            }
            if offset == 0 {
                break;
            }
            offset = offset.saturating_sub(rows);
        }
        screen.set_scrollback(cur);
        (lines, max)
    }

    /// Prompt marks as history indices, oldest first.
    pub fn prompt_marks(&self) -> Vec<usize> {
        self.parser.lock().unwrap().callbacks().marks.indices()
    }

    pub fn set_scroll(&mut self, offset: usize) {
        self.parser
            .lock()
            .unwrap()
            .screen_mut()
            .set_scrollback(offset);
    }

    pub fn reset_scroll(&mut self) {
        let mut parser = self.parser.lock().unwrap();
        if parser.screen().scrollback() != 0 {
            parser.screen_mut().set_scrollback(0);
        }
    }
}

impl Drop for Pane {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
