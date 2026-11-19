use crossterm::event::Event;

pub enum AppEvent {
    Input(Event),
    PtyOutput,
    PtyExit(usize),
}
