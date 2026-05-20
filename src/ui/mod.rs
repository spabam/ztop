// Copyright (c) 2026 Andrea Bodei <info@andreabodei.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

mod bars;
mod table;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::AppState;
use crate::metrics::Snapshot;

const CORE_LABEL_WIDTH: usize = 6; // "Cores " on the first row, blanks after.
const MIN_CORE_INDEX_WIDTH: usize = 2;
const CORE_USAGE_WIDTH: usize = 4; // "100%"
const BAR_START_COL: usize = 34;

// Color palette
const LABEL: Style = Style::new().fg(Color::Cyan);
const BRAND: Style = Style::new().fg(Color::Gray);
const TEMP: Style = Style::new().fg(Color::Yellow);
const TITLE: Style = Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD);

pub fn draw(f: &mut Frame, app: &AppState, snap: &Snapshot) {
    let area = f.area();
    if area.width < 40 {
        let p = Paragraph::new("terminal too narrow (need ≥ 40 cols)")
            .style(Style::default().fg(Color::Red));
        f.render_widget(p, area);
        return;
    }

    let n_cores = snap.cpu_per_core_pct.len().max(1);
    let core_cols = cores_per_row(area.width, n_cores);
    let core_rows = n_cores.div_ceil(core_cols);
    let cpu_panel_h = (core_rows as u16) + 4; // brand + total + cores + 2 borders

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),           // header
            Constraint::Length(cpu_panel_h), // cpu (brand + total + cores)
            Constraint::Length(4),           // memory (ram + swap, merged)
            Constraint::Length(5),           // gpu (brand + util + vram)
            Constraint::Min(8),              // proc table
            Constraint::Length(3),           // footer
        ])
        .split(area);

    draw_header(f, chunks[0], app, snap);
    draw_cpu(f, chunks[1], snap, core_cols);
    draw_memory(f, chunks[2], snap);
    draw_gpu(f, chunks[3], snap);
    table::draw(f, chunks[4], snap, app.top_n, area.width);
    draw_footer(f, chunks[5], app);
}

fn draw_header(f: &mut Frame, area: Rect, app: &AppState, snap: &Snapshot) {
    let uptime = format_uptime(snap.uptime_secs);
    let line = Line::from(vec![
        Span::styled("ZTOP", TITLE),
        Span::raw("   "),
        Span::styled("refresh", LABEL),
        Span::raw(format!(": {:.2}s   ", app.refresh_secs)),
        Span::styled("host", LABEL),
        Span::raw(format!(": {}   ", snap.hostname)),
        Span::styled("uptime", LABEL),
        Span::raw(format!(": {uptime}")),
    ]);
    let p = Paragraph::new(line).block(Block::default().borders(Borders::ALL));
    f.render_widget(p, area);
}

fn draw_cpu(f: &mut Frame, area: Rect, snap: &Snapshot, core_cols: usize) {
    let inner_w = area.width.saturating_sub(2) as usize;
    let cpu_text = format!("   {:5.1}%   ", snap.cpu_total_pct);
    let mut total_spans: Vec<Span<'static>> = vec![
        Span::styled("CPU", LABEL),
        Span::raw(cpu_text.clone()),
        Span::styled("temp", LABEL),
    ];
    let mut prefix_width = "CPU".len() + cpu_text.chars().count() + "temp".len();
    match snap.cpu_temp_c {
        Some(t) => {
            let temp_text = format!("{t:5.1}°C");
            total_spans.push(Span::raw(": "));
            total_spans.push(Span::styled(temp_text.clone(), TEMP));
            prefix_width += ": ".len() + temp_text.chars().count();
        }
        None => {
            let temp_text = ":   n/a";
            total_spans.push(Span::raw(temp_text));
            prefix_width += temp_text.len();
        }
    }
    append_aligned_bar(&mut total_spans, prefix_width, snap.cpu_total_pct, area);

    let mut lines: Vec<Line<'static>> = vec![
        Line::styled(truncate(&snap.cpu_brand, inner_w), BRAND),
        Line::from(total_spans),
    ];

    // Per-core lines pack as many cells as the terminal width allows.
    let index_width = core_index_width(snap.cpu_per_core_pct.len().max(1));
    let mut core_cells: Vec<String> = snap
        .cpu_per_core_pct
        .iter()
        .enumerate()
        .map(|(i, p)| format!("{:>width$}:{:3.0}%", i, p, width = index_width))
        .collect();
    let mut first = true;
    while !core_cells.is_empty() {
        let take = core_cells.len().min(core_cols);
        let drained: Vec<String> = core_cells.drain(..take).collect();
        let prefix: Span<'static> = if first {
            Span::styled("Cores ", LABEL)
        } else {
            Span::raw("      ")
        };
        first = false;
        lines.push(Line::from(vec![prefix, Span::raw(drained.join(" "))]));
    }

    let p = Paragraph::new(lines).block(Block::default().borders(Borders::ALL));
    f.render_widget(p, area);
}

fn draw_memory(f: &mut Frame, area: Rect, snap: &Snapshot) {
    // RAM line
    let ram_used = snap.ram_used_bytes as f32 / 1_073_741_824.0;
    let ram_total = snap.ram_total_bytes.max(1) as f32 / 1_073_741_824.0;
    let ram_pct = (ram_used / ram_total) * 100.0;
    let ram_text = format!("  {ram_used:5.1} / {ram_total:5.1} GB   {ram_pct:5.1}%");
    let mut ram_spans: Vec<Span<'static>> =
        vec![Span::styled("RAM ", LABEL), Span::raw(ram_text.clone())];
    append_aligned_bar(
        &mut ram_spans,
        "RAM ".len() + ram_text.chars().count(),
        ram_pct,
        area,
    );

    // SWAP line
    let swap_total_b = snap.swap_total_bytes;
    let swap_used = snap.swap_used_bytes as f32 / 1_073_741_824.0;
    let swap_total = swap_total_b.max(1) as f32 / 1_073_741_824.0;
    let swap_line = if swap_total_b == 0 {
        Line::from(vec![
            Span::styled("SWAP", LABEL),
            Span::raw("  none configured"),
        ])
    } else {
        let swap_pct = (swap_used / swap_total) * 100.0;
        let swap_text = format!("  {swap_used:5.1} / {swap_total:5.1} GB   {swap_pct:5.1}%");
        let mut swap_spans: Vec<Span<'static>> =
            vec![Span::styled("SWAP", LABEL), Span::raw(swap_text.clone())];
        append_aligned_bar(
            &mut swap_spans,
            "SWAP".len() + swap_text.chars().count(),
            swap_pct,
            area,
        );
        Line::from(swap_spans)
    };

    let lines = vec![Line::from(ram_spans), swap_line];
    let p = Paragraph::new(lines).block(Block::default().borders(Borders::ALL));
    f.render_widget(p, area);
}

fn draw_gpu(f: &mut Frame, area: Rect, snap: &Snapshot) {
    let inner_w = area.width.saturating_sub(2) as usize;
    let lines: Vec<Line<'static>> = match &snap.gpu {
        None => vec![Line::from(vec![
            Span::styled("GPU", LABEL),
            Span::raw("   n/a   (no NVIDIA device or --no-gpu)"),
        ])],
        Some(g) => {
            let vused = g.vram_used_bytes as f32 / 1_073_741_824.0;
            let vtot = g.vram_total_bytes.max(1) as f32 / 1_073_741_824.0;
            let vpct = (vused / vtot) * 100.0;

            let peak_text = g
                .util_peak_pct
                .map(|peak| format!(" peak {peak:3}%"))
                .unwrap_or_default();
            let gpu_text = format!("   {:3}%{peak_text} ", g.util_pct);
            let temp_text = format!("{:3}°C", g.temp_c);
            let mut gpu_spans: Vec<Span<'static>> = vec![
                Span::styled("GPU", LABEL),
                Span::raw(gpu_text.clone()),
                Span::styled("temp", LABEL),
                Span::raw(": "),
                Span::styled(temp_text.clone(), TEMP),
            ];
            append_aligned_bar(
                &mut gpu_spans,
                "GPU".len()
                    + gpu_text.chars().count()
                    + "temp".len()
                    + ": ".len()
                    + temp_text.chars().count(),
                g.util_pct as f32,
                area,
            );

            let vram_text = format!("  {vused:5.1} / {vtot:5.1} GB   {vpct:5.1}%");
            let mut vram_spans: Vec<Span<'static>> =
                vec![Span::styled("VRAM", LABEL), Span::raw(vram_text.clone())];
            append_aligned_bar(
                &mut vram_spans,
                "VRAM".len() + vram_text.chars().count(),
                vpct,
                area,
            );

            vec![
                Line::styled(gpu_title(g, inner_w), BRAND),
                Line::from(gpu_spans),
                Line::from(vram_spans),
            ]
        }
    };
    let p = Paragraph::new(lines).block(Block::default().borders(Borders::ALL));
    f.render_widget(p, area);
}

fn gpu_title(g: &crate::metrics::GpuSnapshot, width: usize) -> String {
    let mut title = g.name.clone();
    if let Some(vram_peak) = g.vram_peak_used_bytes {
        let vram_peak_gb = vram_peak as f32 / 1_073_741_824.0;
        title.push_str(&format!(" | peak VRAM {vram_peak_gb:.1} GB"));
    }
    truncate(&title, width)
}

fn draw_footer(f: &mut Frame, area: Rect, app: &AppState) {
    let paused = if app.paused { "yes" } else { "no" };
    let line = Line::from(vec![
        Span::styled("sort", LABEL),
        Span::raw(format!(": {}   ", app.sort.label())),
        Span::styled("paused", LABEL),
        Span::raw(format!(": {paused}   ")),
        Span::styled("q", LABEL),
        Span::raw(" quit   "),
        Span::styled("c/r/g/v/t", LABEL),
        Span::raw(" sort   "),
        Span::styled("space", LABEL),
        Span::raw(" pause   "),
        Span::styled("+/-", LABEL),
        Span::raw(" rate"),
    ]);
    let p = Paragraph::new(line).block(Block::default().borders(Borders::ALL));
    f.render_widget(p, area);
}

fn format_uptime(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    format!("{h:02}:{m:02}:{s:02}")
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(n.saturating_sub(1)).collect();
        t.push('…');
        t
    }
}

fn append_aligned_bar(spans: &mut Vec<Span<'static>>, prefix_width: usize, pct: f32, area: Rect) {
    spans.push(Span::raw(" ".repeat(bar_padding(prefix_width))));
    spans.extend(bars::bar_spans(pct, bar_width(area)));
}

fn bar_padding(prefix_width: usize) -> usize {
    BAR_START_COL.saturating_sub(prefix_width).max(1)
}

fn bar_width(area: Rect) -> u16 {
    let inner_w = area.width.saturating_sub(2) as usize;
    inner_w.saturating_sub(BAR_START_COL) as u16
}

fn cores_per_row(area_width: u16, n_cores: usize) -> usize {
    let inner_w = area_width.saturating_sub(2) as usize;
    let available = inner_w.saturating_sub(CORE_LABEL_WIDTH);
    let cell_w = core_cell_width(n_cores.max(1));

    // Account for one joining space between adjacent core cells.
    ((available + 1) / (cell_w + 1)).max(1)
}

fn core_cell_width(n_cores: usize) -> usize {
    core_index_width(n_cores) + 1 + CORE_USAGE_WIDTH
}

fn core_index_width(n_cores: usize) -> usize {
    n_cores
        .saturating_sub(1)
        .to_string()
        .len()
        .max(MIN_CORE_INDEX_WIDTH)
}

#[cfg(test)]
mod tests {
    use super::{bar_padding, core_cell_width, cores_per_row, BAR_START_COL};

    #[test]
    fn core_cell_width_grows_for_three_digit_ids() {
        assert_eq!(core_cell_width(8), 7);
        assert_eq!(core_cell_width(128), 8);
    }

    #[test]
    fn cores_per_row_uses_available_width() {
        assert_eq!(cores_per_row(80, 128), 8);
        assert_eq!(cores_per_row(160, 128), 17);
        assert_eq!(128_usize.div_ceil(cores_per_row(160, 128)), 8);
    }

    #[test]
    fn cores_per_row_never_returns_zero() {
        assert_eq!(cores_per_row(10, 128), 1);
    }

    #[test]
    fn metric_bar_prefixes_pad_to_the_same_column() {
        assert_eq!(31 + bar_padding(31), BAR_START_COL);
        assert_eq!(27 + bar_padding(27), BAR_START_COL);
        assert_eq!(34 + bar_padding(34), BAR_START_COL + 1);
    }
}
