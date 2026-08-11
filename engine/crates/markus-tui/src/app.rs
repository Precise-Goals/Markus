//! Application state machine — drives all TUI transitions

use std::path::PathBuf;

use markus_core::{MarkusConfig, ModelInfo, ModelScanner, SystemInfo};

/// Which screen is currently active
#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    MainMenu,
    ModelBrowser,
    Chat,
    SystemDashboard,
    Downloading,
    Settings,
    Quitting,
}

/// Global application state
pub struct App {
    pub mode: AppMode,
    pub config: MarkusConfig,
    pub models: Vec<ModelInfo>,
    pub selected_model: Option<PathBuf>,
    pub system_info: Option<SystemInfo>,
    /// Main menu cursor position
    pub menu_cursor: usize,
    /// Model browser cursor position
    pub model_cursor: usize,
    /// Whether we're currently loading something
    pub loading: bool,
    pub loading_msg: String,
    /// Status bar message
    pub status_msg: String,
    /// Tick count for animations
    pub tick: u64,
    /// Error message (cleared after display)
    pub error: Option<String>,
}

impl App {
    pub fn new(config: MarkusConfig) -> Self {
        Self {
            mode: AppMode::MainMenu,
            config,
            models: vec![],
            selected_model: None,
            system_info: None,
            menu_cursor: 0,
            model_cursor: 0,
            loading: false,
            loading_msg: String::new(),
            status_msg: "Ready".into(),
            tick: 0,
            error: None,
        }
    }

    pub fn tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
    }

    pub fn set_error(&mut self, msg: impl Into<String>) {
        self.error = Some(msg.into());
    }

    pub fn clear_error(&mut self) {
        self.error = None;
    }

    pub fn set_status(&mut self, msg: impl Into<String>) {
        self.status_msg = msg.into();
    }

    pub fn start_loading(&mut self, msg: impl Into<String>) {
        self.loading = true;
        self.loading_msg = msg.into();
    }

    pub fn stop_loading(&mut self) {
        self.loading = false;
        self.loading_msg.clear();
    }

    pub fn go_to(&mut self, mode: AppMode) {
        self.mode = mode;
    }

    pub fn go_back(&mut self) {
        self.mode = AppMode::MainMenu;
    }

    pub fn menu_up(&mut self, len: usize) {
        if len == 0 { return; }
        self.menu_cursor = (self.menu_cursor + len - 1) % len;
    }

    pub fn menu_down(&mut self, len: usize) {
        if len == 0 { return; }
        self.menu_cursor = (self.menu_cursor + 1) % len;
    }

    pub fn model_up(&mut self) {
        let len = self.models.len();
        if len == 0 { return; }
        self.model_cursor = (self.model_cursor + len - 1) % len;
    }

    pub fn model_down(&mut self) {
        let len = self.models.len();
        if len == 0 { return; }
        self.model_cursor = (self.model_cursor + 1) % len;
    }

    pub fn selected_model_info(&self) -> Option<&ModelInfo> {
        self.models.get(self.model_cursor)
    }
}
