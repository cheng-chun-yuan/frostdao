//! Single-line text input component

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

/// Single-line text input widget
#[derive(Clone, Default)]
pub struct TextInput {
    /// Current value
    value: String,
    /// Cursor position
    cursor: usize,
    /// Label displayed above input
    label: String,
    /// Placeholder text when empty
    placeholder: String,
    /// Whether input is numeric only
    numeric: bool,
}

impl TextInput {
    pub fn new(label: &str) -> Self {
        Self {
            label: label.to_string(),
            ..Default::default()
        }
    }

    pub fn with_placeholder(mut self, placeholder: &str) -> Self {
        self.placeholder = placeholder.to_string();
        self
    }

    pub fn with_value(mut self, value: &str) -> Self {
        self.value = value.to_string();
        self.cursor = self.value.len();
        self
    }

    pub fn numeric(mut self) -> Self {
        self.numeric = true;
        self
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn set_value(&mut self, value: &str) {
        self.value = value.to_string();
        self.cursor = self.value.len();
    }

    pub fn clear(&mut self) {
        self.value.clear();
        self.cursor = 0;
    }

    /// Handle key event, returns true if the event was handled
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            // Ctrl+U: Clear line (check before general Char case)
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.clear();
                true
            }
            KeyCode::Char(c) => {
                if self.numeric && !c.is_ascii_digit() {
                    return false;
                }
                self.value.insert(self.cursor, c);
                self.cursor += 1;
                true
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.value.remove(self.cursor);
                }
                true
            }
            KeyCode::Delete => {
                if self.cursor < self.value.len() {
                    self.value.remove(self.cursor);
                }
                true
            }
            KeyCode::Left => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
                true
            }
            KeyCode::Right => {
                if self.cursor < self.value.len() {
                    self.cursor += 1;
                }
                true
            }
            KeyCode::Home => {
                self.cursor = 0;
                true
            }
            KeyCode::End => {
                self.cursor = self.value.len();
                true
            }
            _ => false,
        }
    }

    /// Render the text input
    pub fn render(&self, frame: &mut Frame, area: Rect, focused: bool) {
        let display_value = if self.value.is_empty() {
            Span::styled(&self.placeholder, Style::default().fg(Color::DarkGray))
        } else {
            Span::raw(&self.value)
        };

        let border_color = if focused { Color::Cyan } else { Color::Gray };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(self.label.as_str());

        let paragraph = Paragraph::new(Line::from(display_value)).block(block);

        frame.render_widget(paragraph, area);

        // Show cursor when focused
        if focused && !self.value.is_empty() {
            frame.set_cursor_position((
                area.x + 1 + self.cursor as u16,
                area.y + 1,
            ));
        } else if focused && self.value.is_empty() {
            frame.set_cursor_position((area.x + 1, area.y + 1));
        }
    }

    /// Render with custom style modifiers
    pub fn render_styled(&self, frame: &mut Frame, area: Rect, focused: bool, style: Style) {
        let display_value = if self.value.is_empty() {
            Span::styled(&self.placeholder, Style::default().fg(Color::DarkGray))
        } else {
            Span::styled(&self.value, style)
        };

        let border_color = if focused { Color::Cyan } else { Color::Gray };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(self.label.as_str());

        let paragraph = Paragraph::new(Line::from(display_value)).block(block);

        frame.render_widget(paragraph, area);

        if focused {
            frame.set_cursor_position((
                area.x + 1 + self.cursor as u16,
                area.y + 1,
            ));
        }
    }
}
