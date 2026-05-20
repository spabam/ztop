// Copyright (c) 2026 Andrea Bodei <info@andreabodei.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

use ratatui::{
    style::{Color, Style},
    text::Span,
};

/// Return a horizontal bar as a list of styled spans:
///   `[█████░░░░░]`
/// Fill color is load-graded: green < 50 %, yellow 50–80 %, red ≥ 80 %.
/// Empty cells are dim. Brackets are default colored.
pub fn bar_spans(pct: f32, width: u16) -> Vec<Span<'static>> {
    let width = width as usize;
    if width < 4 {
        return Vec::new();
    }
    let inner = width - 2;
    let p = pct.clamp(0.0, 100.0) / 100.0;
    let filled = (p * inner as f32).round() as usize;
    let empty = inner.saturating_sub(filled);

    let fill_color = if pct < 50.0 {
        Color::Green
    } else if pct < 80.0 {
        Color::Yellow
    } else {
        Color::Red
    };

    vec![
        Span::raw("["),
        Span::styled("█".repeat(filled), Style::default().fg(fill_color)),
        Span::styled("░".repeat(empty), Style::default().fg(Color::DarkGray)),
        Span::raw("]"),
    ]
}
