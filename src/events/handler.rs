use crossterm::event::KeyCode;
use crate::app::App;
use crate::events::app_event::AppEvent;

pub fn handle_event(app: &mut App, event: AppEvent) {
    match event {
        AppEvent::Key(key) => match key.code {
            KeyCode::Char('q') => app.should_quit = true,
            KeyCode::Char('h') | KeyCode::Left  => {}
            KeyCode::Char('j') | KeyCode::Down  => {}
            KeyCode::Char('k') | KeyCode::Up    => {}
            KeyCode::Char('l') | KeyCode::Right => {}
            _ => {}
        },
        AppEvent::Quit => app.should_quit = true,
        _ => {}
    }
}
