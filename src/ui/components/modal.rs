use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub enum ModalKind {
    Confirm,
    Error,
}

pub struct Modal {
    pub title: String,
    pub message: String,
    pub kind: ModalKind,
}

impl Modal {
    pub fn confirm(title: &str, message: &str) -> Self {
        Self { title: title.to_string(), message: message.to_string(), kind: ModalKind::Confirm }
    }

    pub fn error(title: &str, message: &str) -> Self {
        Self { title: title.to_string(), message: message.to_string(), kind: ModalKind::Error }
    }

    pub fn draw(&self, f: &mut Frame<'_>, area: Rect) {
        let popup = centered_rect(60, 9, area);
        f.render_widget(Clear, popup);

        let (border_color, title_color) = match self.kind {
            ModalKind::Confirm => (Color::Yellow, Color::Yellow),
            ModalKind::Error   => (Color::Red,    Color::Red),
        };

        let block = Block::default()
            .title(format!(" {} ", self.title))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title_style(Style::default().fg(title_color).add_modifier(Modifier::BOLD));

        f.render_widget(block, popup);

        let inner = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([Constraint::Min(1), Constraint::Length(1), Constraint::Length(1)])
            .split(popup);

        // Message (word-wrapped to available width)
        let msg_width = inner[0].width.saturating_sub(2) as usize;
        let wrapped = word_wrap(&self.message, msg_width);
        f.render_widget(
            Paragraph::new(wrapped)
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::White)),
            inner[0],
        );

        // Button hint
        let hint = match self.kind {
            ModalKind::Confirm => Line::from(vec![
                Span::styled(" [Y] ", Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::raw(" Confirm   "),
                Span::styled(" [N] / [Esc] ", Style::default().fg(Color::Black).bg(Color::DarkGray).add_modifier(Modifier::BOLD)),
                Span::raw(" Cancel"),
            ]),
            ModalKind::Error => Line::from(vec![
                Span::styled(" [Enter] / [Esc] ", Style::default().fg(Color::Black).bg(Color::DarkGray).add_modifier(Modifier::BOLD)),
                Span::raw(" Close"),
            ]),
        };
        f.render_widget(
            Paragraph::new(hint).alignment(Alignment::Center),
            inner[2],
        );
    }
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect {
        x,
        y,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}

fn word_wrap(text: &str, width: usize) -> String {
    if width == 0 { return text.to_string(); }
    let mut lines = Vec::new();
    for line in text.lines() {
        let mut current = String::new();
        for word in line.split_whitespace() {
            if current.is_empty() {
                current.push_str(word);
            } else if current.len() + 1 + word.len() <= width {
                current.push(' ');
                current.push_str(word);
            } else {
                lines.push(current.clone());
                current = word.to_string();
            }
        }
        if !current.is_empty() { lines.push(current); }
    }
    lines.join("\n")
}
