#![allow(dead_code)]

use ratatui::{layout::Rect, Frame};

pub struct Modal {
    pub title: String,
    pub message: String,
}

impl Modal {
    pub fn new(title: &str, message: &str) -> Self {
        Self {
            title: title.to_string(),
            message: message.to_string(),
        }
    }

    pub fn draw(&self, _f: &mut Frame<'_>, _area: Rect) {
        // TODO: centered confirmation/error dialog
    }
}
