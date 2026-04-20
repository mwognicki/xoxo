#![deny(warnings)]

//! `xoxo` binary entrypoint. Wires CLI subcommands to the daemon and,
//! optionally, the TUI overlay. No business logic lives here.

mod daemon;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::Path;
use xoxo_core::syntax_highlighter::highlight_syntax;

use nerd::tools::read_file::ReadFileTool;
use xoxo_core::config::load_config;

#[derive(Parser)]
#[command(name = "xoxo", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run the daemon headless.
    Daemon,
    /// Run the TUI overlay (embeds the daemon in-process).
    #[cfg(feature = "tui")]
    Tui,
    /// Development tools
    #[command(subcommand)]
    Dev(DevCommand),
}

#[derive(Subcommand)]
enum DevCommand {
    /// Read file contents with optional line range
    ReadFile {
        /// File path (absolute or relative to PWD)
        file_path: String,
        /// Optional line range in format "start:end" (1-indexed, inclusive)
        line_range: Option<String>,
        /// Include noise in output (default: false)
        #[arg(long, default_value_t = false)]
        with_noise: bool,
    },
    /// List available syntax highlighting themes
    ListThemes,
}

#[tokio::main]
async fn main() -> Result<()> {
    load_config();
    let cli = Cli::parse();
    match cli.command {
        Command::Daemon => run_headless_daemon().await,
        #[cfg(feature = "tui")]
        Command::Tui => run_tui().await,
        Command::Dev(dev_cmd) => match dev_cmd {
            DevCommand::ReadFile { file_path, line_range, with_noise } => {
                handle_read_file(file_path, line_range, with_noise).await?;
                Ok(())
            }
            DevCommand::ListThemes => {
                handle_list_themes()?;
                Ok(())
            }
        },
    }
}

/// Handle the list themes command
fn handle_list_themes() -> Result<()> {
    use xoxo_core::syntax_highlighter::list_themes;

    let themes = list_themes();
    for (i, theme) in themes.iter().enumerate() {
        println!("{:>3}. {}", i + 1, theme);
    }
    Ok(())
}

/// Handle the read file command
async fn handle_read_file(
    file_path: String,
    line_range: Option<String>,
    with_noise: bool,
) -> Result<()> {
    let output = match ReadFileTool
        .execute(&file_path, line_range.as_deref(), with_noise)
        .await
    {
        Ok(output) => output,
        Err(err) => {
            eprintln!("Error: {:?}", err);
            std::process::exit(1);
        }
    };

    let content = output["content"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("read_file returned non-string content"))?;
    let total_lines = output["total_lines"]
        .as_u64()
        .ok_or_else(|| anyhow::anyhow!("read_file returned non-numeric total_lines"))?;

    // Get file extension for syntax highlighting
    let extension = Path::new(&file_path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("txt");

    // Apply syntax highlighting
    let highlighted_content = highlight_syntax(extension, content);

    // Print with line numbers
    for (line_num, line) in highlighted_content.lines().enumerate() {
        println!("\x1b[38;5;235m{:>4} |\x1b[0m {}\x1b[0m", line_num + 1, line);
    }
    println!("\n---");
    println!("Total lines in file: {}", total_lines);
    println!("Lines displayed: {}", content.lines().count());

    Ok(())
}

#[cfg(feature = "tui")]
async fn run_tui() -> Result<()> {
    use xoxo_core::bus::{Bus, Command};
    use xoxo_core::chat::structs::{ChatTextMessage, ChatTextRole};
    use xoxo_tui::{App, Tui, draw};
    use crossterm::event;

    let mut tui = Tui::new()?;
    let mut app = App::new();
    let (bus, inbox) = Bus::new();
    let mut events = bus.subscribe_events();
    let _daemon = daemon::spawn_daemon(bus.clone(), inbox);
    tui.enter()?;

    while app.running {
        // Non-blocking poll could be added, but simple blocking read suffices.
        if event::poll(std::time::Duration::from_millis(200))? {
            let ev = event::read()?;
            app.handle_event(ev)?;
        }

        if let Some(message) = app.take_submitted_message() {
            bus.send_command(Command::SubmitUserMessage {
                active_chat_id: app.active_chat_id(),
                message: ChatTextMessage {
                    role: ChatTextRole::User,
                    content: message,
                },
            })
            .await?;
        }

        loop {
            match daemon::ignore_lagged(events.try_recv()) {
                Some(event) => app.handle_bus_event(event),
                None => break,
            }
        }

        // Pass the current layout mode to the draw function.
        let mode = app.layout;
        tui.terminal().draw(|f| draw(f, mode, &app))?;
    }

    tui.exit()?;
    Ok(())
}

async fn run_headless_daemon() -> Result<()> {
    use xoxo_core::bus::Bus;

    let (bus, inbox) = Bus::new();
    daemon::run_daemon_until_shutdown(bus, inbox).await
}
