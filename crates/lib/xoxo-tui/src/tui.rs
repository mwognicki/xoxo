//! Low-level TUI abstractions.

use anyhow::Result;
use crossterm::{
    event::{
        DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

pub struct Tui {
    terminal: Terminal<CrosstermBackend<std::io::Stderr>>,
    mouse_capture_enabled: bool,
}

impl Tui {
    pub fn new() -> Result<Self> {
        let backend = CrosstermBackend::new(std::io::stderr());
        let terminal = Terminal::new(backend)?;
        Ok(Self {
            terminal,
            mouse_capture_enabled: false,
        })
    }

    pub fn enter(&mut self) -> Result<()> {
        enable_raw_mode()?;
        execute!(
            self.terminal.backend_mut(),
            EnterAlternateScreen,
            EnableBracketedPaste,
            EnableMouseCapture
        )?;
        self.mouse_capture_enabled = true;
        Ok(())
    }

    pub fn exit(&mut self) -> Result<()> {
        disable_raw_mode()?;
        if self.mouse_capture_enabled {
            execute!(self.terminal.backend_mut(), DisableMouseCapture)?;
            self.mouse_capture_enabled = false;
        }
        execute!(
            self.terminal.backend_mut(),
            DisableBracketedPaste,
            LeaveAlternateScreen
        )?;
        Ok(())
    }

    /// Synchronises terminal mouse capture with the desired state.
    ///
    /// When disabled, the terminal's native text selection becomes available
    /// again at the cost of in-app scroll-wheel handling.
    pub fn set_mouse_capture(&mut self, enabled: bool) -> Result<()> {
        if self.mouse_capture_enabled == enabled {
            return Ok(());
        }
        if enabled {
            execute!(self.terminal.backend_mut(), EnableMouseCapture)?;
        } else {
            execute!(self.terminal.backend_mut(), DisableMouseCapture)?;
        }
        self.mouse_capture_enabled = enabled;
        Ok(())
    }

    pub fn terminal(&mut self) -> &mut Terminal<CrosstermBackend<std::io::Stderr>> {
        &mut self.terminal
    }
}
