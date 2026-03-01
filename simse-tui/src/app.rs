//! Elm Architecture: Model, Update, View.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};
use simse_ui_core::input::state as input;

/// Application state (the Model).
pub struct App {
    pub input: input::InputState,
    pub messages: Vec<String>,
    pub should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            input: input::InputState::default(),
            messages: vec!["Welcome to SimSE!".into()],
            should_quit: false,
        }
    }
}

/// Messages the app can receive.
pub enum AppMessage {
    CharInput(char),
    Submit,
    Backspace,
    CursorLeft,
    CursorRight,
    Home,
    End,
    Quit,
}

/// Update: pure function from (Model, Message) -> Model.
pub fn update(mut app: App, msg: AppMessage) -> App {
    match msg {
        AppMessage::CharInput(c) => {
            app.input = input::insert(&app.input, &c.to_string());
        }
        AppMessage::Submit => {
            if !app.input.value.is_empty() {
                app.messages.push(format!("> {}", app.input.value));
                app.input = input::InputState::default();
            }
        }
        AppMessage::Backspace => {
            app.input = input::backspace(&app.input);
        }
        AppMessage::CursorLeft => {
            app.input = input::move_left(&app.input, false);
        }
        AppMessage::CursorRight => {
            app.input = input::move_right(&app.input, false);
        }
        AppMessage::Home => {
            app.input = input::move_home(&app.input, false);
        }
        AppMessage::End => {
            app.input = input::move_end(&app.input, false);
        }
        AppMessage::Quit => {
            app.should_quit = true;
        }
    }
    app
}

/// View: render the model to the terminal.
pub fn view(app: &App, frame: &mut Frame) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(frame.area());

    // Message list
    let messages: Vec<Line> = app
        .messages
        .iter()
        .map(|m| Line::from(m.as_str()))
        .collect();
    let messages_widget = Paragraph::new(messages)
        .block(Block::default().borders(Borders::ALL).title("SimSE"));
    frame.render_widget(messages_widget, chunks[0]);

    // Input
    let input_text = if app.input.value.is_empty() {
        vec![Span::styled(
            "Type a message...",
            Style::default().fg(Color::DarkGray),
        )]
    } else {
        vec![Span::raw(&app.input.value)]
    };
    let input_widget = Paragraph::new(Line::from(input_text))
        .block(Block::default().borders(Borders::ALL).title("Input"));
    frame.render_widget(input_widget, chunks[1]);

    // Position cursor
    let cursor_x = chunks[1].x + 1 + app.input.cursor as u16;
    let cursor_y = chunks[1].y + 1;
    frame.set_cursor_position((cursor_x, cursor_y));
}
