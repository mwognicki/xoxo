#![deny(warnings)]

//! `xoxo` binary entrypoint. Wires CLI subcommands to the daemon and,
//! optionally, the TUI overlay. No business logic lives here.

mod daemon;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::sync::Arc;
use uuid::Uuid;

use xoxo_core::config::load_config;
use xoxo_core::storage::bootstrap_storage;

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
    /// Dump a raw persisted chat snapshot from sled before deserializing it
    DumpStoredChat {
        /// Root chat id to inspect
        #[arg(long)]
        chat_id: Option<Uuid>,
        /// Read the last used chat id from sled and dump that chat
        #[arg(long, default_value_t = false)]
        last_used: bool,
    },
    /// Purge the entire sled storage directory under ~/.xoxo/data
    PurgeStorage,
}

#[tokio::main]
async fn main() -> Result<()> {
    load_config();
    let storage = Arc::new(bootstrap_storage()?);
    let cli = Cli::parse();
    match cli.command {
        Command::Daemon => run_headless_daemon(storage).await,
        #[cfg(feature = "tui")]
        Command::Tui => run_tui(storage).await,
        Command::Dev(dev_cmd) => match dev_cmd {
            DevCommand::DumpStoredChat { chat_id, last_used } => {
                handle_dump_stored_chat(storage.as_ref(), chat_id, last_used)?;
                Ok(())
            }
            DevCommand::PurgeStorage => {
                handle_purge_storage(storage)?;
                Ok(())
            }
        },
    }
}

/// Dump a raw persisted chat snapshot from sled without deserializing it.
fn handle_dump_stored_chat(
    storage: &xoxo_core::storage::Storage,
    chat_id: Option<Uuid>,
    last_used: bool,
) -> Result<()> {
    let resolved_chat_id = if last_used {
        storage.last_used_chat_id()?
    } else {
        chat_id
    };

    let Some(chat_id) = resolved_chat_id else {
        println!("No chat id resolved.");
        return Ok(());
    };

    println!("Chat ID: {chat_id}");
    if last_used {
        println!("Resolved from: last_used_chat_id");
    }

    match storage.load_raw_chat(chat_id)? {
        Some(raw_chat) => {
            println!("{raw_chat}");
        }
        None => {
            println!("No stored chat snapshot found for {chat_id}");
        }
    }

    Ok(())
}

/// Purge the entire sled storage directory.
fn handle_purge_storage(storage: Arc<xoxo_core::storage::Storage>) -> Result<()> {
    let storage = Arc::try_unwrap(storage)
        .map_err(|_| anyhow::anyhow!("failed to acquire exclusive storage handle for purge"))?;
    let path = storage.path().to_path_buf();
    storage.purge()?;
    println!("Purged storage at {}", path.display());
    Ok(())
}

#[cfg(feature = "tui")]
async fn run_tui(storage: Arc<xoxo_core::storage::Storage>) -> Result<()> {
    use xoxo_core::bus::{Bus, Command};
    use xoxo_core::chat::structs::{ChatTextMessage, ChatTextRole};
    use xoxo_tui::{App, Tui, draw};
    use crossterm::event;

    let mut tui = Tui::new()?;
    let restored_chat = match storage.last_used_chat_id()? {
        Some(chat_id) => match storage.load_chat(chat_id) {
            Ok(chat) => chat,
            Err(error) => {
                eprintln!(
                    "Warning: failed to restore persisted chat {chat_id}: {error}"
                );
                None
            }
        },
        None => None,
    };
    let mut app = App::new(restored_chat);
    let (bus, inbox) = Bus::new();
    let mut events = bus.subscribe_events();
    let _daemon = daemon::spawn_daemon(bus.clone(), inbox, storage.clone());
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
                Some(event) => {
                    let root_chat_id = *event.path.root_id();
                    app.handle_bus_event(event);
                    if let Some(chat) = storage.load_chat(root_chat_id)? {
                        app.sync_chat_summary(&chat);
                    }
                }
                None => break,
            }
        }

        tui.set_mouse_capture(app.mouse_capture_enabled)?;

        // Pass the current layout mode to the draw function.
        let mode = app.layout;
        tui.terminal().draw(|f| draw(f, mode, &app))?;
    }

    tui.exit()?;
    Ok(())
}

async fn run_headless_daemon(storage: Arc<xoxo_core::storage::Storage>) -> Result<()> {
    use xoxo_core::bus::Bus;

    let (bus, inbox) = Bus::new();
    daemon::run_daemon_until_shutdown(bus, inbox, storage).await
}
