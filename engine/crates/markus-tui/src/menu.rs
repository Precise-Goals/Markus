//! Menu definitions

#[derive(Debug, Clone)]
pub struct MenuItem {
    pub label: &'static str,
    pub key: &'static str,
    pub description: &'static str,
}

pub fn main_menu_items() -> Vec<MenuItem> {
    vec![
        MenuItem { label: "Chat",     key: "c", description: "Start an interactive AI conversation" },
        MenuItem { label: "Models",   key: "m", description: "Browse, download, and manage models" },
        MenuItem { label: "System",   key: "s", description: "Hardware info, RAM management, settings" },
        MenuItem { label: "Pull",     key: "p", description: "Download a new model from HuggingFace" },
        MenuItem { label: "Serve",    key: "v", description: "Start OpenAI-compatible API server" },
        MenuItem { label: "Quit",     key: "q", description: "Exit Markus" },
    ]
}

pub fn model_actions() -> Vec<MenuItem> {
    vec![
        MenuItem { label: "Chat",   key: "Enter", description: "Start chatting with this model" },
        MenuItem { label: "Serve",  key: "s",     description: "Serve this model as API server" },
        MenuItem { label: "Info",   key: "i",     description: "Show detailed model metadata" },
        MenuItem { label: "Delete", key: "d",     description: "Remove model from disk" },
        MenuItem { label: "Back",   key: "Esc",   description: "Return to main menu" },
    ]
}
