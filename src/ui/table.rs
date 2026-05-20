// Copyright (c) 2026 Andrea Bodei <info@andreabodei.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table},
    Frame,
};

use crate::metrics::Snapshot;

pub fn draw(f: &mut Frame, area: Rect, snap: &Snapshot, top_n: usize, term_width: u16) {
    let show_user = term_width >= 60;
    let show_gpu = term_width >= 70;
    let show_vram = term_width >= 76;

    let mut header = vec!["PID", "CPU%", "RAM%"];
    if show_user {
        header.insert(1, "USER");
    }
    if show_gpu {
        header.push("GPU%");
    }
    if show_vram {
        header.push("VRAM");
    }
    header.push("COMMAND");

    let header_row = Row::new(header.iter().map(|h| Cell::from(*h))).style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );

    let rows: Vec<Row> = snap
        .procs
        .iter()
        .take(top_n)
        .map(|r| {
            let mut cells: Vec<Cell> = vec![Cell::from(r.pid.to_string())];
            if show_user {
                cells.push(Cell::from(truncate(&r.user, 10)));
            }
            cells.push(Cell::from(format!("{:5.1}", r.cpu_pct)));
            cells.push(Cell::from(format!("{:5.1}", r.ram_pct)));
            if show_gpu {
                cells.push(Cell::from(match r.gpu_pct {
                    Some(p) => format!("{p:3}"),
                    None => "—".into(),
                }));
            }
            if show_vram {
                cells.push(Cell::from(format_vram(r.vram_bytes)));
            }
            cells.push(Cell::from(truncate(&r.command, 64)));
            Row::new(cells)
        })
        .collect();

    let mut widths: Vec<Constraint> = vec![Constraint::Length(6)];
    if show_user {
        widths.push(Constraint::Length(10));
    }
    widths.push(Constraint::Length(6)); // cpu
    widths.push(Constraint::Length(6)); // ram
    if show_gpu {
        widths.push(Constraint::Length(5));
    }
    if show_vram {
        widths.push(Constraint::Length(8));
    }
    widths.push(Constraint::Min(10));

    let table = Table::new(rows, widths).header(header_row).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Top Consuming"),
    );
    f.render_widget(table, area);
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

fn format_vram(bytes: u64) -> String {
    if bytes == 0 {
        return "—".into();
    }
    let mb = bytes as f64 / 1_048_576.0;
    if mb < 1024.0 {
        format!("{mb:.0}MB")
    } else {
        format!("{:.1}GB", mb / 1024.0)
    }
}
