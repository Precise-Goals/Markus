//! Chat state — conversation history and streaming output buffer

use markus_core::pipeline::ChatMessage;

pub struct ChatState {
    pub history: Vec<ChatMessage>,
    pub input: String,
    pub cursor_pos: usize,
    /// Currently streaming assistant response
    pub streaming_buf: String,
    pub is_streaming: bool,
    pub system_prompt: String,
    pub scroll_offset: usize,
    pub stats: Option<ChatStats>,
}

#[derive(Debug, Clone)]
pub struct ChatStats {
    pub tokens_generated: u32,
    pub elapsed_ms: u64,
    pub tokens_per_sec: f64,
}

impl ChatState {
    pub fn new(system_prompt: String) -> Self {
        Self {
            history: vec![],
            input: String::new(),
            cursor_pos: 0,
            streaming_buf: String::new(),
            is_streaming: false,
            system_prompt,
            scroll_offset: 0,
            stats: None,
        }
    }

    pub fn push_char(&mut self, c: char) {
        self.input.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
    }

    pub fn backspace(&mut self) {
        if self.cursor_pos > 0 {
            let mut pos = self.cursor_pos - 1;
            while !self.input.is_char_boundary(pos) { pos -= 1; }
            self.input.remove(pos);
            self.cursor_pos = pos;
        }
    }

    pub fn take_input(&mut self) -> String {
        self.cursor_pos = 0;
        std::mem::take(&mut self.input)
    }

    pub fn start_streaming(&mut self, user_msg: String) {
        self.history.push(ChatMessage { role: "user".into(), content: user_msg });
        self.streaming_buf.clear();
        self.is_streaming = true;
        self.stats = None;
    }

    pub fn append_stream_token(&mut self, token: &str) {
        self.streaming_buf.push_str(token);
    }

    pub fn finish_streaming(&mut self, stats: ChatStats) {
        let response = std::mem::take(&mut self.streaming_buf);
        if !response.is_empty() {
            self.history.push(ChatMessage { role: "assistant".into(), content: response });
        }
        self.is_streaming = false;
        self.stats = Some(stats);
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
        self.streaming_buf.clear();
        self.is_streaming = false;
        self.scroll_offset = 0;
        self.stats = None;
    }

    pub fn messages_for_inference(&self) -> Vec<ChatMessage> {
        let mut msgs = vec![
            ChatMessage { role: "system".into(), content: self.system_prompt.clone() }
        ];
        msgs.extend(self.history.clone());
        msgs
    }

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset += 1;
    }
}
