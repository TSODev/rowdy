use std::thread;
use std::time::Duration;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    widgets::{Block, Borders, Paragraph},
    Terminal,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Initialiser le terminal en mode brut (TUI)
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 2. Dessiner le message d'attente
    terminal.draw(|f| {
        let size = f.size();
        let message = Paragraph::new("\n\n   🤠 Rowdy - Bientôt ici votre administrateur de BDD en terminal !\n\n   Fermeture automatique...")
            .block(Block::default().title(" [ Rowdy v0.1.0 ] ").borders(Borders::ALL));
        f.render_widget(message, size);
    })?;

    // 3. Maintenir l'affichage pendant 3 secondes
    thread::sleep(Duration::from_secs(3));

    // 4. Nettoyer et restaurer le terminal d'origine
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}