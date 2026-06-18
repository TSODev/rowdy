use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use crate::app::{App, AppState};
use crate::ui::screens;
use crate::ui::components::modal;

pub fn draw(f: &mut Frame<'_>, app: &mut App) {
    let tab_bar_height = if app.tabs.len() > 1 { 1 } else { 0 };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(tab_bar_height),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(f.size());

    let tab_area   = chunks[0];
    let main_area  = chunks[1];
    let status_area = chunks[2];

    // ── Tab bar (only when 2+ tabs) ───────────────────────────────────────────
    if app.tabs.len() > 1 {
        let bg = Style::default().bg(Color::DarkGray);
        let active_style = Style::default()
            .fg(Color::Black)
            .bg(Color::Yellow)
            .add_modifier(Modifier::BOLD);
        let inactive_style = Style::default().fg(Color::White).bg(Color::DarkGray);

        let mut spans: Vec<Span> = Vec::new();
        for (i, tab) in app.tabs.iter().enumerate() {
            let label = format!(" [{}] {} ", i + 1, tab.display_name());
            if i == app.active_tab {
                spans.push(Span::styled(label, active_style));
            } else {
                spans.push(Span::styled(label, inactive_style));
            }
            spans.push(Span::styled(" ", bg));
        }
        f.render_widget(Paragraph::new(Line::from(spans)).style(bg), tab_area);
    }

    // ── Main content ──────────────────────────────────────────────────────────
    let idx = app.active_tab;
    let state = app.tabs[idx].state.clone();
    match state {
        AppState::Connection =>
            screens::connection::ConnectionScreen::draw(f, &mut app.tabs[idx].connection_screen, main_area),
        AppState::TableList =>
            screens::table_list::TableListScreen::draw(f, &mut app.tabs[idx].table_list_screen, main_area),
        AppState::DataGrid =>
            screens::data_grid::DataGridScreen::draw(f, &mut app.tabs[idx].data_grid_screen, main_area),
        AppState::FkGrid =>
            screens::data_grid::DataGridScreen::draw(f, &mut app.tabs[idx].fk_grid_screen, main_area),
        AppState::SqlResultGrid =>
            screens::data_grid::DataGridScreen::draw(f, &mut app.tabs[idx].sql_result_grid_screen, main_area),
        AppState::EditRecord =>
            screens::edit_record::EditRecordScreen::draw(f, &mut app.tabs[idx].edit_record_screen, main_area),
        AppState::SqlEditor =>
            screens::sql_editor::SqlEditorScreen::draw(f, &mut app.tabs[idx].sql_editor_screen, main_area),
        AppState::ErdGraph =>
            screens::erd_graph::ErdGraphScreen::draw(f, &mut app.tabs[idx].erd_graph_screen, main_area),
    }

    // ── Status bar ────────────────────────────────────────────────────────────
    crate::ui::components::status_bar::draw(f, status_area, app);

    // ── Modal ─────────────────────────────────────────────────────────────────
    if let Some(ref m) = app.tabs[idx].modal {
        modal::Modal::draw(m, f, f.size());
    }

    // ── Status message TTL ────────────────────────────────────────────────────
    if app.tabs[idx].status_message_ttl > 0 {
        app.tabs[idx].status_message_ttl -= 1;
        if app.tabs[idx].status_message_ttl == 0 {
            app.tabs[idx].status_message = None;
        }
    }
}
