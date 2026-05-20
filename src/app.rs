// Copyright (c) 2026 Andrea Bodei <info@andreabodei.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::time::Duration;

use crate::cli::{Cli, SortArg};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SortMode {
    Cpu,
    Ram,
    Gpu,
    Vram,
    Total,
}

impl SortMode {
    pub fn label(self) -> &'static str {
        match self {
            SortMode::Cpu => "cpu",
            SortMode::Ram => "ram",
            SortMode::Gpu => "gpu",
            SortMode::Vram => "vram",
            SortMode::Total => "total",
        }
    }
}

impl From<SortArg> for SortMode {
    fn from(s: SortArg) -> Self {
        match s {
            SortArg::Cpu => SortMode::Cpu,
            SortArg::Ram => SortMode::Ram,
            SortArg::Gpu => SortMode::Gpu,
            SortArg::Vram => SortMode::Vram,
            SortArg::Total => SortMode::Total,
        }
    }
}

pub struct AppState {
    pub sort: SortMode,
    pub paused: bool,
    pub refresh_secs: f64,
    pub top_n: usize,
}

impl AppState {
    pub fn from_cli(cli: &Cli) -> Self {
        Self {
            sort: cli.sort.into(),
            paused: false,
            refresh_secs: cli.refresh,
            top_n: cli.top,
        }
    }

    pub fn refresh_interval(&self) -> Duration {
        Duration::from_millis((self.refresh_secs * 1000.0) as u64)
    }

    pub fn bump_refresh(&mut self, delta: f64) {
        let new = (self.refresh_secs + delta).clamp(0.25, 5.0);
        self.refresh_secs = (new * 4.0).round() / 4.0;
    }
}

#[cfg(test)]
mod tests {
    use super::AppState;

    #[test]
    fn refresh_bump_clamps_and_snaps_to_quarters() {
        let mut app = AppState {
            sort: super::SortMode::Cpu,
            paused: false,
            refresh_secs: 1.0,
            top_n: 10,
        };

        app.bump_refresh(0.26);
        assert_eq!(app.refresh_secs, 1.25);

        app.bump_refresh(-10.0);
        assert_eq!(app.refresh_secs, 0.25);

        app.bump_refresh(10.0);
        assert_eq!(app.refresh_secs, 5.0);
    }
}
