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
            screens::data_grid::DataGridScreen::draw(f),
        crate::app::AppState::SqlEditor =>
            screens::sql_editor::SqlEditorScreen::draw(f),
        crate::app::AppState::Quit => {}
    }
}
