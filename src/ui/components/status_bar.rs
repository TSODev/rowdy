use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use crate::app::{App, AppState};

pub fn draw(f: &mut Frame<'_>, area: Rect, app: &App) {
    let mode = match app.state {
        AppState::Connection    => "CONNECTION",
        AppState::TableList     => "TABLES",
        AppState::DataGrid      => "DATA GRID",
        AppState::FkGrid        => "FK VIEW",
        AppState::EditRecord    => "EDIT",
        AppState::SqlEditor     => "SQL EDITOR",
        AppState::SqlResultGrid => "QUERY RESULT",
    };

    let (dot, dot_color) = if app.active_client.is_some() {
        ("● ", Color::Green)
    } else {
        ("○ ", Color::Red)
    };

    let db_info = app.connected_db_info.as_deref().unwrap_or("—");
    let db_info_display: String = if db_info.chars().count() > 45 {
        let s: String = db_info.chars().take(44).collect();
        format!("{s}…")
    } else {
        db_info.to_string()
    };

    let row_count: Option<u64> = match app.state {
        AppState::DataGrid      => app.data_grid_screen.total_count,
        AppState::FkGrid        => app.fk_grid_screen.total_count,
        AppState::SqlResultGrid => app.sql_result_grid_screen.total_count,
        _ => None,
    };

    let bg = Style::default().bg(Color::DarkGray);
    let mode_style = Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD);
    let dot_style  = Style::default().fg(dot_color).bg(Color::DarkGray);
    let info_style = Style::default().fg(Color::White).bg(Color::DarkGray);
    let dim_style  = Style::default().fg(Color::Gray).bg(Color::DarkGray);

    let mut spans = vec![
        Span::styled(format!(" {mode} "), mode_style),
        Span::styled("  ", bg),
        Span::styled(dot, dot_style),
        Span::styled(db_info_display, info_style),
    ];

    if let Some(count) = row_count {
        spans.push(Span::styled(format!("  [{count} rows]"), dim_style));
    }

    // Flash message — right-aligned, fills remaining space
    if let Some((ref msg, is_err)) = app.status_message {
        let flash_style = if is_err {
            Style::default().fg(Color::Red).bg(Color::DarkGray).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Green).bg(Color::DarkGray)
        };
        spans.push(Span::styled("  ", bg));
        spans.push(Span::styled(format!("{msg} "), flash_style));
    }

    f.render_widget(
        Paragraph::new(Line::from(spans)).style(bg),
        area,
    );
}
