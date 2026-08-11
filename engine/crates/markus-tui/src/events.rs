//! Event loop — keyboard input + tick timer

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;

#[derive(Debug)]
pub enum AppEvent {
    Key(KeyEvent),
    Tick,
    Resize(u16, u16),
}

pub struct EventHandler {
    tick_rate_ms: u64,
}

impl EventHandler {
    pub fn new(tick_rate_ms: u64) -> Self {
        Self { tick_rate_ms }
    }

    pub fn next(&self) -> anyhow::Result<AppEvent> {
        let timeout = Duration::from_millis(self.tick_rate_ms);
        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(k) => Ok(AppEvent::Key(k)),
                Event::Resize(w, h) => Ok(AppEvent::Resize(w, h)),
                _ => Ok(AppEvent::Tick),
            }
        } else {
            Ok(AppEvent::Tick)
        }
    }
}

/// Convenience: check if a key event matches a simple char
pub fn key_char(event: &KeyEvent, c: char) -> bool {
    event.code == KeyCode::Char(c)
}
