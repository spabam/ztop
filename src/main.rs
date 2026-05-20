// Copyright (c) 2026 Andrea Bodei <info@andreabodei.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

mod app;
mod cli;
mod metrics;
mod ui;

use anyhow::{Context, Result};
use clap::Parser;
use crossterm::{
    cursor::Show,
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{
    io,
    time::{Duration, Instant},
};

use app::{AppState, SortMode};
use cli::Cli;
use metrics::Collector;

struct TerminalCleanup;

impl Drop for TerminalCleanup {
    fn drop(&mut self) {
        let mut stdout = io::stdout();
        disable_raw_mode().ok();
        execute!(stdout, LeaveAlternateScreen, DisableMouseCapture, Show).ok();
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut app = AppState::from_cli(&cli);
    let mut collector = Collector::new(&cli)?;

    let mut stdout = io::stdout();
    enable_raw_mode().context("enable raw mode")?;
    let _cleanup = TerminalCleanup;
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    run(&mut terminal, &mut app, &mut collector)
}

fn run<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut AppState,
    collector: &mut Collector,
) -> Result<()> {
    let mut last_tick = Instant::now();
    collector.sample_gpu();
    let mut snapshot = collector.collect(app);
    terminal.draw(|f| ui::draw(f, app, &snapshot))?;

    loop {
        let now = Instant::now();
        let tick = app.refresh_interval();
        let elapsed = now.duration_since(last_tick);
        let refresh_wait = if app.paused {
            Duration::from_millis(100)
        } else if elapsed >= tick {
            Duration::ZERO
        } else {
            (tick - elapsed).min(Duration::from_millis(100))
        };
        let poll_for = collector
            .gpu_sample_wait(now)
            .map(|gpu_wait| refresh_wait.min(gpu_wait))
            .unwrap_or(refresh_wait);

        let mut needs_draw = false;
        if event::poll(poll_for)? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Char('c') => {
                        app.sort = SortMode::Cpu;
                        snapshot.sort_processes(app.sort);
                        needs_draw = true;
                    }
                    KeyCode::Char('r') => {
                        app.sort = SortMode::Ram;
                        snapshot.sort_processes(app.sort);
                        needs_draw = true;
                    }
                    KeyCode::Char('g') => {
                        app.sort = SortMode::Gpu;
                        snapshot.sort_processes(app.sort);
                        needs_draw = true;
                    }
                    KeyCode::Char('v') => {
                        app.sort = SortMode::Vram;
                        snapshot.sort_processes(app.sort);
                        needs_draw = true;
                    }
                    KeyCode::Char('t') => {
                        app.sort = SortMode::Total;
                        snapshot.sort_processes(app.sort);
                        needs_draw = true;
                    }
                    KeyCode::Char(' ') => {
                        app.paused = !app.paused;
                        needs_draw = true;
                    }
                    KeyCode::Char('+') | KeyCode::Char('=') => {
                        app.bump_refresh(0.25);
                        needs_draw = true;
                    }
                    KeyCode::Char('-') | KeyCode::Char('_') => {
                        app.bump_refresh(-0.25);
                        needs_draw = true;
                    }
                    _ => {}
                }
            }
        }

        let now = Instant::now();
        collector.sample_gpu_if_due(now);
        let now = Instant::now();
        if !app.paused && now.duration_since(last_tick) >= app.refresh_interval() {
            snapshot = collector.collect(app);
            last_tick = now;
            needs_draw = true;
        }

        if needs_draw {
            terminal.draw(|f| ui::draw(f, app, &snapshot))?;
        }
    }
}
