#![allow(dead_code)]

use ratatui::{layout::Rect, Frame};

pub struct StatusBar;

impl StatusBar {
    pub fn draw(_f: &mut Frame<'_>, _area: Rect) {
        // TODO: connection status, current mode, keybindings hint
    }
}
