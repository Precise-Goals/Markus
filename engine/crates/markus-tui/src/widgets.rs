//! Custom TUI widgets (loading bar, spinner, stats gauge)

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style, Stylize},
    text::{Line, Span},
    widgets::{StatefulWidget, Widget},
};

const ACCENT: Color = Color::Rgb(0, 190, 210);
const BRAND: Color = Color::Rgb(220, 50, 50);
const DIM: Color = Color::Rgb(100, 100, 110);
const SUCCESS: Color = Color::Rgb(80, 200, 120);

/// Animated spinner widget
pub struct Spinner {
    pub frame: u64,
    pub message: String,
}

impl Widget for Spinner {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let frames = ["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"];
        let sp = frames[(self.frame / 2) as usize % frames.len()];
        let text = format!("  {}  {}", sp, self.message);
        buf.set_string(area.x, area.y, &text, Style::new().fg(ACCENT));
    }
}

/// Download progress bar
pub struct DownloadBar {
    pub downloaded: u64,
    pub total: u64,
    pub filename: String,
}

impl Widget for DownloadBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 2 || area.width < 10 { return; }

        let pct = if self.total > 0 {
            (self.downloaded as f64 / self.total as f64 * 100.0) as u64
        } else {
            0
        };

        let mb_dl = self.downloaded as f64 / 1024.0 / 1024.0;
        let mb_total = self.total as f64 / 1024.0 / 1024.0;

        let label = if self.total > 0 {
            format!("  {}  {:.1}/{:.1} MB  {}%", self.filename, mb_dl, mb_total, pct)
        } else {
            format!("  {}  {:.1} MB", self.filename, mb_dl)
        };

        buf.set_string(area.x, area.y, &label, Style::new().fg(ACCENT));

        // Draw bar
        let bar_width = area.width.saturating_sub(4) as u64;
        let filled = if self.total > 0 {
            (bar_width * self.downloaded / self.total) as u16
        } else {
            0
        };

        let y = area.y + 1;
        buf.set_string(area.x, y, "  [", Style::new().fg(DIM));
        for x in 0..bar_width as u16 {
            let ch = if x < filled { '█' } else { '░' };
            let color = if x < filled { SUCCESS } else { DIM };
            buf.set_string(area.x + 3 + x, y, &ch.to_string(), Style::new().fg(color));
        }
        buf.set_string(area.x + 3 + bar_width as u16, y, "]", Style::new().fg(DIM));
    }
}
