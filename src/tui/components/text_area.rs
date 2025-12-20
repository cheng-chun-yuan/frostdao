//! Multi-line text area component for JSON paste

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

/// Multi-line text area widget
#[derive(Clone, Default)]
pub struct TextArea {
    /// Content lines
    lines: Vec<String>,
    /// Current line
    cursor_line: usize,
    /// Current column
    cursor_col: usize,
    /// Scroll offset
    scroll_offset: usize,
    /// Label
    label: String,
    /// Placeholder
    placeholder: String,
}

impl TextArea {
    pub fn new(label: &str) -> Self {
        Self {
            lines: vec![String::new()],
            label: label.to_string(),
            ..Default::default()
        }
    }

    pub fn with_placeholder(mut self, placeholder: &str) -> Self {
        self.placeholder = placeholder.to_string();
        self
    }

    pub fn content(&self) -> String {
        self.lines.join("\n")
    }

    pub fn set_content(&mut self, content: &str) {
        self.lines = content.lines().map(|s| s.to_string()).collect();
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.cursor_line = 0;
        self.cursor_col = 0;
        self.scroll_offset = 0;
    }

    pub fn clear(&mut self) {
        self.lines = vec![String::new()];
        self.cursor_line = 0;
        self.cursor_col = 0;
        self.scroll_offset = 0;
    }

    pub fn is_empty(&self) -> bool {
        self.lines.len() == 1 && self.lines[0].is_empty()
    }

    /// Handle paste (Ctrl+V or direct paste)
    pub fn handle_paste(&mut self, text: &str) {
        for ch in text.chars() {
            if ch == '\n' {
                let current_line = &mut self.lines[self.cursor_line];
                let rest = current_line.split_off(self.cursor_col);
                self.cursor_line += 1;
                self.lines.insert(self.cursor_line, rest);
                self.cursor_col = 0;
            } else if ch != '\r' {
                self.lines[self.cursor_line].insert(self.cursor_col, ch);
                self.cursor_col += 1;
            }
        }
    }

    /// Handle key event
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char(c) => {
                // Check for Ctrl+V paste (handled separately by caller)
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    return false;
                }
                self.lines[self.cursor_line].insert(self.cursor_col, c);
                self.cursor_col += 1;
                true
            }
            KeyCode::Enter => {
                let current_line = &mut self.lines[self.cursor_line];
                let rest = current_line.split_off(self.cursor_col);
                self.cursor_line += 1;
                self.lines.insert(self.cursor_line, rest);
                self.cursor_col = 0;
                true
            }
            KeyCode::Backspace => {
                if self.cursor_col > 0 {
                    self.cursor_col -= 1;
                    self.lines[self.cursor_line].remove(self.cursor_col);
                } else if self.cursor_line > 0 {
                    let current = self.lines.remove(self.cursor_line);
                    self.cursor_line -= 1;
                    self.cursor_col = self.lines[self.cursor_line].len();
                    self.lines[self.cursor_line].push_str(&current);
                }
                true
            }
            KeyCode::Delete => {
                if self.cursor_col < self.lines[self.cursor_line].len() {
                    self.lines[self.cursor_line].remove(self.cursor_col);
                } else if self.cursor_line < self.lines.len() - 1 {
                    let next = self.lines.remove(self.cursor_line + 1);
                    self.lines[self.cursor_line].push_str(&next);
                }
                true
            }
            KeyCode::Left => {
                if self.cursor_col > 0 {
                    self.cursor_col -= 1;
                } else if self.cursor_line > 0 {
                    self.cursor_line -= 1;
                    self.cursor_col = self.lines[self.cursor_line].len();
                }
                true
            }
            KeyCode::Right => {
                if self.cursor_col < self.lines[self.cursor_line].len() {
                    self.cursor_col += 1;
                } else if self.cursor_line < self.lines.len() - 1 {
                    self.cursor_line += 1;
                    self.cursor_col = 0;
                }
                true
            }
            KeyCode::Up => {
                if self.cursor_line > 0 {
                    self.cursor_line -= 1;
                    self.cursor_col = self.cursor_col.min(self.lines[self.cursor_line].len());
                }
                true
            }
            KeyCode::Down => {
                if self.cursor_line < self.lines.len() - 1 {
                    self.cursor_line += 1;
                    self.cursor_col = self.cursor_col.min(self.lines[self.cursor_line].len());
                }
                true
            }
            KeyCode::Home => {
                self.cursor_col = 0;
                true
            }
            KeyCode::End => {
                self.cursor_col = self.lines[self.cursor_line].len();
                true
            }
            _ => false,
        }
    }

    /// Render the text area
    pub fn render(&self, frame: &mut Frame, area: Rect, focused: bool) {
        let border_color = if focused { Color::Cyan } else { Color::Gray };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(self.label.as_str());

        let inner = block.inner(area);
        let visible_height = inner.height as usize;

        // Adjust scroll to keep cursor visible
        let mut scroll = self.scroll_offset;
        if self.cursor_line < scroll {
            scroll = self.cursor_line;
        } else if self.cursor_line >= scroll + visible_height {
            scroll = self.cursor_line - visible_height + 1;
        }

        let content = if self.is_empty() {
            vec![Line::from(Span::styled(
                &self.placeholder,
                Style::default().fg(Color::DarkGray),
            ))]
        } else {
            self.lines
                .iter()
                .skip(scroll)
                .take(visible_height)
                .map(|line| Line::from(line.as_str()))
                .collect()
        };

        let paragraph = Paragraph::new(content)
            .block(block)
            .wrap(Wrap { trim: false });

        frame.render_widget(paragraph, area);

        // Show cursor when focused
        if focused {
            let cursor_y = area.y + 1 + (self.cursor_line - scroll) as u16;
            let cursor_x = area.x + 1 + self.cursor_col as u16;
            if cursor_y < area.y + area.height - 1 && cursor_x < area.x + area.width - 1 {
                frame.set_cursor_position((cursor_x, cursor_y));
            }
        }
    }
}
