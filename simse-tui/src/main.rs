//! SimSE TUI — Terminal interface for SimSE.

mod app;

use std::io;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};

use app::{App, AppMessage, update, view};

fn main() -> io::Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run app
    let result = run_app(&mut terminal);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let mut app = App::new();

    loop {
        terminal.draw(|frame| view(&app, frame))?;

        if let Event::Key(key) = event::read()? {
            let msg = match (key.code, key.modifiers) {
                (KeyCode::Char('c'), KeyModifiers::CONTROL) => AppMessage::Quit,
                (KeyCode::Char(c), _) => AppMessage::CharInput(c),
                (KeyCode::Enter, _) => AppMessage::Submit,
                (KeyCode::Backspace, _) => AppMessage::Backspace,
                (KeyCode::Left, _) => AppMessage::CursorLeft,
                (KeyCode::Right, _) => AppMessage::CursorRight,
                (KeyCode::Home, _) => AppMessage::Home,
                (KeyCode::End, _) => AppMessage::End,
                _ => continue,
            };

            app = update(app, msg);

            if app.should_quit {
                return Ok(());
            }
        }
    }
}
