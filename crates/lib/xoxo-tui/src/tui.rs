//! Low-level TUI abstractions.

use anyhow::Result;
use crossterm::{
    event::{DisableBracketedPaste, EnableBracketedPaste},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

pub struct Tui {
    terminal: Terminal<CrosstermBackend<std::io::Stderr>>,
}

impl Tui {
    pub fn new() -> Result<Self> {
        let backend = CrosstermBackend::new(std::io::stderr());
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }

    pub fn enter(&mut self) -> Result<()> {
        enable_raw_mode()?;
        execute!(
            self.terminal.backend_mut(),
            EnterAlternateScreen,
            EnableBracketedPaste
        )?;
        Ok(())
    }

    pub fn exit(&mut self) -> Result<()> {
        disable_raw_mode()?;
        execute!(
            self.terminal.backend_mut(),
            DisableBracketedPaste,
            LeaveAlternateScreen
        )?;
        Ok(())
    }

    pub fn terminal(&mut self) -> &mut Terminal<CrosstermBackend<std::io::Stderr>> {
        &mut self.terminal
    }
}
