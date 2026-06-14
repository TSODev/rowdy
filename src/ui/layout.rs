use ratatui::Frame;
use crate::app::App;
use crate::ui::screens;

pub fn draw(f: &mut Frame<'_>, app: &mut App) {
    match app.state {
        crate::app::AppState::Connection =>
            screens::connection::ConnectionScreen::draw(f, &mut app.connection_screen),
        crate::app::AppState::TableList =>
            screens::table_list::TableListScreen::draw(f, &mut app.table_list_screen),
        crate::app::AppState::DataGrid =>
            screens::data_grid::DataGridScreen::draw(f, &mut app.data_grid_screen),
        crate::app::AppState::EditRecord =>
            screens::edit_record::EditRecordScreen::draw(f, &mut app.edit_record_screen),
        crate::app::AppState::SqlEditor =>
            screens::sql_editor::SqlEditorScreen::draw(f, &mut app.sql_editor_screen),
        crate::app::AppState::Quit => {}
    }
}
