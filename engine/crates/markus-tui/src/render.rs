//! Ratatui render functions — draws all screens

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{
        Block, BorderType, Borders, Clear, Gauge, List, ListItem,
        Padding, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap,
    },
};

use crate::app::{App, AppMode};
use crate::chat::ChatState;
use crate::menu::main_menu_items;

// ── Color palette ──────────────────────────────────────────────────────────────
const BRAND:   Color = Color::Rgb(220, 50,  50);   // Markus red
const ACCENT:  Color = Color::Rgb(0,   190, 210);  // Cyan accent
const DIM:     Color = Color::Rgb(100, 100, 110);  // Dim grey
const SUCCESS: Color = Color::Rgb(80,  200, 120);  // Green
const WARN:    Color = Color::Rgb(255, 200, 50);   // Yellow
const FG:      Color = Color::Rgb(230, 230, 235);  // Near-white

/// Main render dispatcher
pub fn render(app: &App, chat: Option<&ChatState>, frame: &mut Frame) {
    match &app.mode {
        AppMode::MainMenu    => render_main_menu(app, frame),
        AppMode::ModelBrowser => render_model_browser(app, frame),
        AppMode::Chat        => {
            if let Some(chat) = chat {
                render_chat(app, chat, frame);
            }
        }
        AppMode::SystemDashboard => render_system(app, frame),
        AppMode::Settings    => render_settings(app, frame),
        AppMode::Downloading => render_downloading(app, frame),
        AppMode::Quitting    => {}
    }

    // Overlay loading spinner if active
    if app.loading {
        render_loading_overlay(app, frame);
    }

    // Overlay error if present
    if let Some(err) = &app.error {
        render_error_overlay(err, frame);
    }
}

// ── ASCII Banner ───────────────────────────────────────────────────────────────
fn ascii_banner() -> Vec<Line<'static>> {
    vec![
        Line::from(Span::styled("  ███╗   ███╗ █████╗ ██████╗ ██╗  ██╗██╗   ██╗███████╗", Style::new().fg(BRAND).bold())),
        Line::from(Span::styled("  ████╗ ████║██╔══██╗██╔══██╗██║ ██╔╝██║   ██║██╔════╝", Style::new().fg(BRAND).bold())),
        Line::from(Span::styled("  ██╔████╔██║███████║██████╔╝█████╔╝ ██║   ██║███████╗", Style::new().fg(BRAND).bold())),
        Line::from(Span::styled("  ██║╚██╔╝██║██╔══██║██╔══██╗██╔═██╗ ██║   ██║╚════██║", Style::new().fg(BRAND).bold())),
        Line::from(Span::styled("  ██║ ╚═╝ ██║██║  ██║██║  ██║██║  ██╗╚██████╔╝███████║", Style::new().fg(BRAND).bold())),
        Line::from(Span::styled("  ╚═╝     ╚═╝╚═╝  ╚═╝╚═╝  ╚═╝╚═╝  ╚═╝ ╚═════╝ ╚══════╝", Style::new().fg(BRAND).bold())),
    ]
}

// ── Main Menu ──────────────────────────────────────────────────────────────────
fn render_main_menu(app: &App, frame: &mut Frame) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),   // banner
            Constraint::Length(1),   // subtitle
            Constraint::Length(1),   // spacer
            Constraint::Min(12),     // menu
            Constraint::Length(2),   // status bar
        ])
        .split(area);

    // Banner
    let banner = Paragraph::new(ascii_banner())
        .alignment(Alignment::Left);
    frame.render_widget(banner, chunks[0]);

    // Subtitle
    let subtitle = Paragraph::new(Line::from(vec![
        Span::styled("  Pure Rust Inference Engine  ·  ", Style::new().fg(DIM)),
        Span::styled("v3.0.0", Style::new().fg(ACCENT)),
        Span::styled("  ·  No llama.cpp  ·  ", Style::new().fg(DIM)),
        Span::styled(
            format!("{} models found", app.models.len()),
            Style::new().fg(SUCCESS),
        ),
    ]));
    frame.render_widget(subtitle, chunks[1]);

    // Menu items
    let items = main_menu_items();
    let list_items: Vec<ListItem> = items.iter().enumerate().map(|(i, item)| {
        let is_selected = i == app.menu_cursor;
        if is_selected {
            ListItem::new(Line::from(vec![
                Span::styled("  ▸  ", Style::new().fg(BRAND).bold()),
                Span::styled(
                    format!("{:<10}", item.label),
                    Style::new().fg(WARN).bold(),
                ),
                Span::styled("  ", Style::default()),
                Span::styled(item.description, Style::new().fg(ACCENT)),
            ]))
        } else {
            ListItem::new(Line::from(vec![
                Span::styled("     ", Style::default()),
                Span::styled(
                    format!("{:<10}", item.label),
                    Style::new().fg(FG),
                ),
                Span::styled("  ", Style::default()),
                Span::styled(item.description, Style::new().fg(DIM)),
            ]))
        }
    }).collect();

    let menu_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(BRAND))
        .title(Span::styled(" Main Menu ", Style::new().fg(ACCENT).bold()))
        .padding(Padding::new(1, 1, 0, 0));

    let menu_list = List::new(list_items).block(menu_block);
    frame.render_widget(menu_list, chunks[3]);

    // Status bar
    let status_detail = format!("  ·  {}", app.status_msg);
    let status = Paragraph::new(Line::from(vec![
        Span::styled("  ↑↓/jk ", Style::new().fg(WARN).bold()),
        Span::styled("Navigate  ", Style::new().fg(DIM)),
        Span::styled("Enter ", Style::new().fg(WARN).bold()),
        Span::styled("Select  ", Style::new().fg(DIM)),
        Span::styled("q ", Style::new().fg(WARN).bold()),
        Span::styled("Quit  ", Style::new().fg(DIM)),
        Span::styled("1-6 ", Style::new().fg(WARN).bold()),
        Span::styled("Jump  ", Style::new().fg(DIM)),
        Span::styled(&status_detail, Style::new().fg(DIM)),
    ]));
    frame.render_widget(status, chunks[4]);
}

// ── Model Browser ──────────────────────────────────────────────────────────────
fn render_model_browser(app: &App, frame: &mut Frame) {
    let area = frame.area();

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    // Left: model list
    let list_items: Vec<ListItem> = app.models.iter().enumerate().map(|(i, m)| {
        let is_sel = i == app.model_cursor;
        let size = m.size_display();
        let _fmt = m.format.label();

        if is_sel {
            ListItem::new(Line::from(vec![
                Span::styled(" ▸ ", Style::new().fg(BRAND).bold()),
                Span::styled(
                    format!("{}", m.name),
                    Style::new().fg(WARN).bold(),
                ),
                Span::styled(format!("  {}", size), Style::new().fg(SUCCESS)),
            ]))
        } else {
            ListItem::new(Line::from(vec![
                Span::styled("   ", Style::default()),
                Span::styled(&m.name, Style::new().fg(FG)),
                Span::styled(format!("  {}", size), Style::new().fg(DIM)),
            ]))
        }
    }).collect();

    let list_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(BRAND))
        .title(Span::styled(
            format!(" Models ({}) ", app.models.len()),
            Style::new().fg(ACCENT).bold()
        ));

    let list = List::new(list_items).block(list_block);
    frame.render_widget(list, chunks[0]);

    // Right: model detail panel
    if let Some(m) = app.selected_model_info() {
        let mut lines: Vec<Line> = vec![
            Line::from(Span::styled("  Model Details", Style::new().fg(ACCENT).bold())),
            Line::from(""),
        ];

        let add_field = |lines: &mut Vec<Line>, key: &'static str, val: String| {
            lines.push(Line::from(vec![
                Span::styled(format!("  {:<18}", key), Style::new().fg(DIM)),
                Span::styled(val, Style::new().fg(FG)),
            ]));
        };

        add_field(&mut lines, "Name:", m.name.clone());
        add_field(&mut lines, "Format:", m.format.label().to_string());
        add_field(&mut lines, "Size:", m.size_display());
        if let Some(arch) = &m.architecture {
            add_field(&mut lines, "Architecture:", arch.clone());
        }
        if let Some(ctx) = m.context_length {
            add_field(&mut lines, "Context:", format!("{} tokens", ctx));
        }
        if let Some(layers) = m.layer_count {
            add_field(&mut lines, "Layers:", layers.to_string());
        }
        if let Some(heads) = m.head_count {
            add_field(&mut lines, "Heads:", heads.to_string());
        }
        if let Some(vocab) = m.vocab_size {
            add_field(&mut lines, "Vocab:", format!("{} tokens", vocab));
        }
        if let Some(params) = m.parameter_count_b {
            add_field(&mut lines, "Parameters:", format!("{:.1}B", params));
        }
        if let Some(ver) = m.gguf_version {
            add_field(&mut lines, "GGUF Version:", ver.to_string());
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("  Path:", Style::new().fg(DIM))));
        lines.push(Line::from(Span::styled(
            format!("  {}", m.path.display()),
            Style::new().fg(ACCENT),
        )));

        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled("  Enter", Style::new().fg(WARN).bold()),
            Span::styled(" Chat  ", Style::new().fg(DIM)),
            Span::styled("s", Style::new().fg(WARN).bold()),
            Span::styled(" Serve  ", Style::new().fg(DIM)),
            Span::styled("d", Style::new().fg(BRAND).bold()),
            Span::styled(" Delete  ", Style::new().fg(DIM)),
            Span::styled("Esc", Style::new().fg(WARN).bold()),
            Span::styled(" Back", Style::new().fg(DIM)),
        ]));

        let detail_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(BRAND))
            .title(Span::styled(" Details ", Style::new().fg(ACCENT).bold()));

        let detail = Paragraph::new(lines).block(detail_block).wrap(Wrap { trim: false });
        frame.render_widget(detail, chunks[1]);
    }
}

// ── Chat ───────────────────────────────────────────────────────────────────────
pub fn render_chat(app: &App, chat: &ChatState, frame: &mut Frame) {
    let area = frame.area();

    let model_name = app.selected_model.as_ref()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "No model".into());

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),   // header
            Constraint::Min(6),      // chat history
            Constraint::Length(3),   // input box
            Constraint::Length(1),   // status
        ])
        .split(area);

    // Header
    let header = Paragraph::new(Line::from(vec![
        Span::styled("  ▸ CHAT  ", Style::new().fg(BRAND).bold()),
        Span::styled(&model_name, Style::new().fg(ACCENT)),
        if chat.is_streaming {
            Span::styled("  ● streaming...", Style::new().fg(WARN))
        } else {
            Span::styled("  ● ready", Style::new().fg(SUCCESS))
        },
    ]));
    frame.render_widget(header, chunks[0]);

    // Chat history
    let mut lines: Vec<Line> = vec![];
    for msg in &chat.history {
        match msg.role.as_str() {
            "user" => {
                lines.push(Line::from(Span::styled("  You:", Style::new().fg(ACCENT).bold())));
                for text_line in msg.content.lines() {
                    lines.push(Line::from(Span::styled(
                        format!("  {}", text_line),
                        Style::new().fg(FG),
                    )));
                }
                lines.push(Line::from(""));
            }
            "assistant" => {
                lines.push(Line::from(Span::styled("  Markus:", Style::new().fg(BRAND).bold())));
                for text_line in msg.content.lines() {
                    lines.push(Line::from(Span::styled(
                        format!("  {}", text_line),
                        Style::new().fg(FG),
                    )));
                }
                // Stats
                if let Some(stats) = &chat.stats {
                    lines.push(Line::from(Span::styled(
                        format!("  [{} tokens · {:.1} t/s · {}ms]",
                            stats.tokens_generated, stats.tokens_per_sec, stats.elapsed_ms),
                        Style::new().fg(DIM),
                    )));
                }
                lines.push(Line::from(""));
            }
            _ => {}
        }
    }

    // Streaming buffer
    if chat.is_streaming && !chat.streaming_buf.is_empty() {
        lines.push(Line::from(Span::styled("  Markus:", Style::new().fg(BRAND).bold())));
        for text_line in chat.streaming_buf.lines() {
            lines.push(Line::from(Span::styled(
                format!("  {}", text_line),
                Style::new().fg(FG),
            )));
        }
        // Blinking cursor
        let blink = if app.tick % 8 < 4 { "▋" } else { " " };
        lines.push(Line::from(Span::styled(blink, Style::new().fg(ACCENT))));
    }

    let history_block = Block::default()
        .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
        .border_style(Style::new().fg(DIM));

    let history = Paragraph::new(lines.clone())
        .block(history_block)
        .wrap(Wrap { trim: false })
        .scroll((chat.scroll_offset as u16, 0));
    frame.render_widget(history, chunks[1]);

    // Input box
    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(if chat.is_streaming { DIM } else { ACCENT }))
        .title(Span::styled(
            " Message (Enter to send, /clear /help /exit) ",
            Style::new().fg(DIM),
        ));
    let input = Paragraph::new(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(&chat.input, Style::new().fg(FG)),
        Span::styled("▋", Style::new().fg(ACCENT)),
    ])).block(input_block);
    frame.render_widget(input, chunks[2]);

    // Status
    let status = Paragraph::new(Line::from(vec![
        Span::styled("  ↑↓ ", Style::new().fg(WARN).bold()),
        Span::styled("Scroll  ", Style::new().fg(DIM)),
        Span::styled("Ctrl+C ", Style::new().fg(WARN).bold()),
        Span::styled("Exit  ", Style::new().fg(DIM)),
        Span::styled("Ctrl+L ", Style::new().fg(WARN).bold()),
        Span::styled("Clear  ", Style::new().fg(DIM)),
    ]));
    frame.render_widget(status, chunks[3]);
}

// ── System Dashboard ───────────────────────────────────────────────────────────
fn render_system(app: &App, frame: &mut Frame) {
    let area = frame.area();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(BRAND))
        .title(Span::styled(" System Dashboard ", Style::new().fg(ACCENT).bold()));

    let mut lines: Vec<Line> = vec![Line::from("")];

    if let Some(info) = &app.system_info {
        let add = |lines: &mut Vec<Line>, key: &str, val: &str| {
            lines.push(Line::from(vec![
                Span::styled(format!("  {:<20}", key), Style::new().fg(DIM)),
                Span::styled(val.to_string(), Style::new().fg(FG).bold()),
            ]));
        };

        lines.push(Line::from(Span::styled("  ─── CPU ──────────────────────────", Style::new().fg(BRAND))));
        add(&mut lines, "Model:", &info.cpu_model);
        add(&mut lines, "Physical cores:", &info.cpu_cores.to_string());
        add(&mut lines, "Logical threads:", &info.cpu_threads.to_string());

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("  ─── Memory ────────────────────────", Style::new().fg(BRAND))));
        add(&mut lines, "Total RAM:", &info.ram_display());
        add(&mut lines, "Available RAM:", &info.available_ram_display());

        if !info.gpu_info.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled("  ─── GPU ──────────────────────────", Style::new().fg(BRAND))));
            for gpu in &info.gpu_info {
                add(&mut lines, "Name:", &gpu.name);
                if let Some(vram) = gpu.vram_total_mb {
                    add(&mut lines, "VRAM:", &format!("{}MB", vram));
                }
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("  ─── Platform ─────────────────────", Style::new().fg(BRAND))));
        add(&mut lines, "OS:", &info.os);
        add(&mut lines, "Architecture:", &info.arch);
    } else {
        lines.push(Line::from(Span::styled("  Loading system info...", Style::new().fg(DIM))));
    }

    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("  f ", Style::new().fg(WARN).bold()),
        Span::styled("Free RAM  ", Style::new().fg(DIM)),
        Span::styled("r ", Style::new().fg(WARN).bold()),
        Span::styled("Refresh  ", Style::new().fg(DIM)),
        Span::styled("Esc ", Style::new().fg(WARN).bold()),
        Span::styled("Back", Style::new().fg(DIM)),
    ]));

    let p = Paragraph::new(lines).block(block).wrap(Wrap { trim: false });
    frame.render_widget(p, area);
}

// ── Settings ──────────────────────────────────────────────────────────────────
fn render_settings(app: &App, frame: &mut Frame) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(BRAND))
        .title(Span::styled(" Settings ", Style::new().fg(ACCENT).bold()));

    let cfg = &app.config;
    let lines: Vec<Line> = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(format!("  {:<22}", "Threads:"), Style::new().fg(DIM)),
            Span::styled(cfg.threads.to_string(), Style::new().fg(FG).bold()),
        ]),
        Line::from(vec![
            Span::styled(format!("  {:<22}", "Context Size:"), Style::new().fg(DIM)),
            Span::styled(cfg.ctx_size.to_string(), Style::new().fg(FG).bold()),
        ]),
        Line::from(vec![
            Span::styled(format!("  {:<22}", "Temperature:"), Style::new().fg(DIM)),
            Span::styled(cfg.temperature.to_string(), Style::new().fg(FG).bold()),
        ]),
        Line::from(vec![
            Span::styled(format!("  {:<22}", "GPU Layers:"), Style::new().fg(DIM)),
            Span::styled(cfg.gpu_layers.to_string(), Style::new().fg(FG).bold()),
        ]),
        Line::from(vec![
            Span::styled(format!("  {:<22}", "Server Port:"), Style::new().fg(DIM)),
            Span::styled(cfg.server_port.to_string(), Style::new().fg(FG).bold()),
        ]),
        Line::from(vec![
            Span::styled(format!("  {:<22}", "Flash Attention:"), Style::new().fg(DIM)),
            Span::styled(cfg.flash_attn.to_string(), Style::new().fg(SUCCESS).bold()),
        ]),
        Line::from(""),
        Line::from(Span::styled("  Edit ~/.config/markus/config.toml to modify settings", Style::new().fg(DIM))),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Esc ", Style::new().fg(WARN).bold()),
            Span::styled("Back", Style::new().fg(DIM)),
        ]),
    ];

    let p = Paragraph::new(lines).block(block);
    frame.render_widget(p, frame.area());
}

// ── Downloading ────────────────────────────────────────────────────────────────
fn render_downloading(app: &App, frame: &mut Frame) {
    let area = frame.area();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(BRAND))
        .title(Span::styled(" Downloading ", Style::new().fg(ACCENT).bold()));

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(&app.loading_msg, Style::new().fg(FG))),
        Line::from(""),
        Line::from(Span::styled("  Press Ctrl+C to cancel", Style::new().fg(DIM))),
    ];

    let p = Paragraph::new(lines).block(block).alignment(Alignment::Left);
    frame.render_widget(p, area);
}

// ── Overlays ──────────────────────────────────────────────────────────────────
fn render_loading_overlay(app: &App, frame: &mut Frame) {
    let area = frame.area();
    let spinner = ["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"];
    let sp = spinner[(app.tick / 2) as usize % spinner.len()];

    let popup = centered_rect(50, 5, area);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(ACCENT))
        .border_type(BorderType::Rounded);

    let text = Paragraph::new(Line::from(vec![
        Span::styled(format!("  {}  ", sp), Style::new().fg(ACCENT).bold()),
        Span::styled(&app.loading_msg, Style::new().fg(FG)),
    ])).block(block).alignment(Alignment::Left);

    frame.render_widget(text, popup);
}

fn render_error_overlay(err: &str, frame: &mut Frame) {
    let area = frame.area();
    let popup = centered_rect(60, 7, area);
    frame.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::new().fg(BRAND))
        .border_type(BorderType::Rounded)
        .title(Span::styled(" Error ", Style::new().fg(BRAND).bold()));

    let text = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(format!("  {}", err), Style::new().fg(FG))),
        Line::from(""),
        Line::from(Span::styled("  Press any key to dismiss", Style::new().fg(DIM))),
    ]).block(block).wrap(Wrap { trim: false });

    frame.render_widget(text, popup);
}

fn centered_rect(percent_x: u16, height: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - height.min(100)) / 2),
            Constraint::Length(height),
            Constraint::Percentage((100 - height.min(100)) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
