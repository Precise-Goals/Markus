//! TUI runner — main event loop using Ratatui + Crossterm

use std::io;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use tokio::sync::mpsc;

use markus_core::{
    MarkusConfig, ModelScanner, SystemInfo,
    pipeline::{ChatMessage, GenerationConfig, TokenEvent},
    GenerationPipeline,
};
use markus_tui::{
    app::{App, AppMode},
    chat::{ChatState, ChatStats},
    events::{AppEvent, EventHandler},
    render,
};

pub async fn run(config: MarkusConfig) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal, config).await;

    // Cleanup terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    result
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    config: MarkusConfig,
) -> Result<()> {
    let mut app = App::new(config.clone());
    let mut chat_state = ChatState::new(config.system_prompt.clone());
    let events = EventHandler::new(80); // ~12fps tick

    // Initial model scan (non-blocking via spawn_blocking)
    app.start_loading("Scanning for models...".to_string());
    let models = tokio::task::spawn_blocking(|| {
        ModelScanner::new().scan(false)
    }).await?;
    app.models = models;
    app.stop_loading();
    app.set_status(format!("Ready — {} models found", app.models.len()));

    // Token stream channel for chat streaming
    let (tok_tx, mut tok_rx) = mpsc::channel::<TokenEvent>(256);
    let mut pipeline: Option<GenerationPipeline> = None;

    loop {
        // Drain any pending token events
        while let Ok(event) = tok_rx.try_recv() {
            match event {
                TokenEvent::Token(t) => chat_state.append_stream_token(&t),
                TokenEvent::Done { tokens_generated, elapsed_ms } => {
                    let tps = tokens_generated as f64 / (elapsed_ms as f64 / 1000.0);
                    chat_state.finish_streaming(ChatStats {
                        tokens_generated,
                        elapsed_ms,
                        tokens_per_sec: tps,
                    });
                    app.set_status(format!("{} tokens at {:.1} t/s", tokens_generated, tps));
                }
                TokenEvent::Error(e) => {
                    chat_state.is_streaming = false;
                    app.set_error(e);
                }
            }
        }

        // Draw frame
        terminal.draw(|f| {
            render::render(&app, Some(&chat_state), f);
        })?;

        app.tick();

        // Get next event (non-blocking — returns Tick if nothing)
        match events.next()? {
            AppEvent::Tick => {}

            AppEvent::Resize(_, _) => {}

            AppEvent::Key(key) => {
                // If there's an error overlay, any key dismisses it
                if app.error.is_some() {
                    app.clear_error();
                    continue;
                }

                // Global quit: Ctrl+C always exits
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    break;
                }

                match &app.mode {
                    AppMode::MainMenu => handle_main_menu_key(&mut app, key.code),

                    AppMode::ModelBrowser => handle_model_browser_key(&mut app, key.code),

                    AppMode::Chat => {
                        handle_chat_key(
                            &mut app,
                            &mut chat_state,
                            &mut pipeline,
                            key.code,
                            key.modifiers,
                            &tok_tx,
                            &config,
                        ).await;
                    }

                    AppMode::SystemDashboard => match key.code {
                        KeyCode::Esc | KeyCode::Char('q') => app.go_back(),
                        KeyCode::Char('f') => {
                            let killed = markus_core::system::kill_inference_processes();
                            let actions = markus_core::system::drop_kernel_caches();
                            app.set_status(format!("Freed memory — killed {} processes", killed.len()));
                        }
                        KeyCode::Char('r') => {
                            let info = SystemInfo::collect();
                            app.system_info = Some(info);
                        }
                        _ => {}
                    },

                    AppMode::Settings => match key.code {
                        KeyCode::Esc | KeyCode::Char('q') => app.go_back(),
                        _ => {}
                    },

                    AppMode::Downloading => {
                        // Download in progress — Ctrl+C handled above
                    }

                    AppMode::Quitting => break,
                }
            }
        }
    }

    Ok(())
}

fn handle_main_menu_key(app: &mut App, key: KeyCode) {
    let n = markus_tui::menu::main_menu_items().len();
    match key {
        KeyCode::Char('q') | KeyCode::Char('Q') => app.go_to(AppMode::Quitting),
        KeyCode::Up | KeyCode::Char('k') => app.menu_up(n),
        KeyCode::Down | KeyCode::Char('j') => app.menu_down(n),
        KeyCode::Enter => {
            match app.menu_cursor {
                0 => { // Chat
                    app.go_to(AppMode::ModelBrowser);
                    app.set_status("Select a model to chat with");
                }
                1 => { // Models
                    app.go_to(AppMode::ModelBrowser);
                    app.set_status("Model browser — Enter to chat, s to serve");
                }
                2 => { // System
                    let info = SystemInfo::collect();
                    app.system_info = Some(info);
                    app.go_to(AppMode::SystemDashboard);
                }
                3 => { // Pull — TODO: show pull sub-menu
                    app.set_status("Use 'markus-engine pull <model>' from CLI to download");
                }
                4 => { // Serve
                    app.go_to(AppMode::ModelBrowser);
                    app.set_status("Select a model to serve as API");
                }
                5 => { // Quit
                    app.go_to(AppMode::Quitting);
                }
                _ => {}
            }
        }
        KeyCode::Char(c) if c.is_ascii_digit() => {
            let idx = c as usize - '1' as usize;
            if idx < n { app.menu_cursor = idx; }
        }
        _ => {}
    }
}

fn handle_model_browser_key(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.go_back();
            app.set_status("Ready");
        }
        KeyCode::Up | KeyCode::Char('k') => app.model_up(),
        KeyCode::Down | KeyCode::Char('j') => app.model_down(),
        KeyCode::Enter => {
            if let Some(m) = app.selected_model_info() {
                app.selected_model = Some(m.path.clone());
                app.go_to(AppMode::Chat);
                app.set_status("Chat mode — type your message and press Enter");
            }
        }
        KeyCode::Char('d') => {
            if let Some(m) = app.selected_model_info() {
                let path = m.path.clone();
                let _ = std::fs::remove_file(&path);
                ModelScanner::new().invalidate_cache();
                let models = ModelScanner::new().scan(true);
                let n = models.len();
                app.models = models;
                app.model_cursor = app.model_cursor.min(n.saturating_sub(1));
                app.set_status("Model removed");
            }
        }
        _ => {}
    }
}

async fn handle_chat_key(
    app: &mut App,
    chat: &mut ChatState,
    pipeline: &mut Option<GenerationPipeline>,
    key: KeyCode,
    mods: KeyModifiers,
    tok_tx: &mpsc::Sender<TokenEvent>,
    config: &MarkusConfig,
) {
    if chat.is_streaming {
        return; // ignore input while streaming
    }

    match key {
        KeyCode::Esc => {
            app.go_to(AppMode::ModelBrowser);
            chat.clear_history();
        }
        KeyCode::Char('l') if mods.contains(KeyModifiers::CONTROL) => {
            chat.clear_history();
            app.set_status("History cleared");
        }
        KeyCode::Up => chat.scroll_up(),
        KeyCode::Down => chat.scroll_down(),
        KeyCode::Backspace => chat.backspace(),
        KeyCode::Char(c) => chat.push_char(c),
        KeyCode::Enter => {
            let input = chat.input.trim().to_string();
            if input.is_empty() { return; }

            // Handle slash commands
            match input.as_str() {
                "/clear" => {
                    chat.clear_history();
                    app.set_status("History cleared");
                    return;
                }
                "/exit" | "/quit" => {
                    app.go_to(AppMode::ModelBrowser);
                    return;
                }
                _ => {}
            }

            // Ensure pipeline is loaded
            if pipeline.is_none() {
                if let Some(model_path) = app.selected_model.clone() {
                    let model_name = model_path.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    app.start_loading(format!("Loading {}...", model_name));
                    match GenerationPipeline::load(&model_path, config).await {
                        Ok(p) => {
                            *pipeline = Some(p);
                            app.stop_loading();
                        }
                        Err(e) => {
                            app.stop_loading();
                            app.set_error(format!("Failed to load model: {}", e));
                            return;
                        }
                    }
                } else {
                    app.set_error("No model selected".to_string());
                    return;
                }
            }

            let taken = chat.take_input();
            chat.start_streaming(taken);
            let messages = chat.messages_for_inference();

            let gen_config = GenerationConfig::from_markus_config(config);
            let tx = tok_tx.clone();

            if let Some(p) = pipeline {
                p.chat_stream(&messages, &gen_config, tx).await;
            }
        }
        _ => {}
    }
}
