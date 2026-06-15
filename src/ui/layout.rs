use ratatui::{layout::{Constraint, Direction, Layout}, Frame};
use crate::app::App;
use crate::ui::screens;

pub fn draw(f: &mut Frame<'_>, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(f.size());
    let main_area = chunks[0];
    let status_area = chunks[1];

    match app.state {
        crate::app::AppState::Connection =>
            screens::connection::ConnectionScreen::draw(f, &mut app.connection_screen, main_area),
        crate::app::AppState::TableList =>
            screens::table_list::TableListScreen::draw(f, &mut app.table_list_screen, main_area),
        crate::app::AppState::DataGrid =>
            screens::data_grid::DataGridScreen::draw(f, &mut app.data_grid_screen, main_area),
        crate::app::AppState::FkGrid =>
            screens::data_grid::DataGridScreen::draw(f, &mut app.fk_grid_screen, main_area),
        crate::app::AppState::SqlResultGrid =>
            screens::data_grid::DataGridScreen::draw(f, &mut app.sql_result_grid_screen, main_area),
        crate::app::AppState::EditRecord =>
            screens::edit_record::EditRecordScreen::draw(f, &mut app.edit_record_screen, main_area),
        crate::app::AppState::SqlEditor =>
            screens::sql_editor::SqlEditorScreen::draw(f, &mut app.sql_editor_screen, main_area),
    }

    crate::ui::components::status_bar::draw(f, status_area, app);

    if app.status_message_ttl > 0 {
        app.status_message_ttl -= 1;
        if app.status_message_ttl == 0 {
            app.status_message = None;
        }
    }
}
