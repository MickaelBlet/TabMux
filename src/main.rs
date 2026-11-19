mod app;
mod event;
mod input;
mod layout;
mod marks;
mod mouse;
mod pane;
mod search;
mod selection;
mod shell;
mod tab;
mod ui;
mod view;
mod workspace;

use std::io::stdout;
use std::sync::mpsc;
use std::thread;

use anyhow::Result;
use crossterm::event::{
    DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
};
use crossterm::execute;
use ratatui::layout::Rect;

use crate::app::App;
use crate::event::AppEvent;
use crate::workspace::Workspace;

const USAGE: &str = "Usage: tabmux [-w|--workspace <file.toml>]";

fn parse_args() -> Result<Option<Workspace>> {
    let mut args = std::env::args().skip(1);
    let mut ws = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "-w" | "--workspace" => {
                let path = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("{a} needs a file"))?;
                ws = Some(Workspace::load(std::path::Path::new(&path))?);
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            _ => anyhow::bail!("unknown argument: {a}\n{USAGE}"),
        }
    }
    Ok(ws)
}

fn main() -> Result<()> {
    let workspace = parse_args()?;
    let mut terminal = ratatui::init();
    execute!(stdout(), EnableMouseCapture, EnableBracketedPaste)?;

    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = execute!(stdout(), DisableMouseCapture, DisableBracketedPaste);
        prev_hook(info);
    }));

    let result = run(&mut terminal, workspace);

    let _ = execute!(stdout(), DisableMouseCapture, DisableBracketedPaste);
    ratatui::restore();
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal, workspace: Option<Workspace>) -> Result<()> {
    let (tx, rx) = mpsc::channel();

    let input_tx = tx.clone();
    thread::spawn(move || {
        while let Ok(ev) = crossterm::event::read() {
            if input_tx.send(AppEvent::Input(ev)).is_err() {
                break;
            }
        }
    });

    let size = terminal.size()?;
    let mut app = App::new(Rect::new(0, 0, size.width, size.height), tx, workspace)?;

    while !app.quit {
        app.refresh();
        terminal.draw(|f| ui::draw(f, &app))?;
        let ev = if app.bell_pending() {
            match rx.recv_timeout(ui::BLINK) {
                Ok(ev) => ev,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(e) => return Err(e.into()),
            }
        } else {
            rx.recv()?
        };
        app.handle(ev)?;
        // Coalesce pending events before the next redraw.
        while let Ok(ev) = rx.try_recv() {
            app.handle(ev)?;
            if app.quit {
                break;
            }
        }
    }
    Ok(())
}
