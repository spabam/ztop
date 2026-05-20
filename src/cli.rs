// Copyright (c) 2026 Andrea Bodei <info@andreabodei.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

use clap::{Parser, ValueEnum};

const LONG_ABOUT: &str = "\
A real-time terminal dashboard for CPU, RAM, swap, GPU, VRAM, and the top
resource-consuming processes. Designed for Linux workstations, GPU servers,
AI research machines, and developers who need a focused, fast monitor.

Features:
  - CPU model, total + per-core utilization, package temperature
  - RAM and swap used/total/percentage (merged panel)
  - NVIDIA GPU model, utilization, temperature, VRAM used/total/percentage
  - 250 ms GPU sampling with a rolling peak hold for short utilization spikes
  - Top-N processes with PID, user, CPU%, RAM%, GPU%, VRAM, command
  - Per-process VRAM via NVML; per-process GPU% when permitted (root or CAP_SYS_ADMIN)
  - 1-second refresh by default, configurable 0.25-5.0s
  - Top-N process table, configurable 1-200 rows
  - Single static binary, no libsensors or nvidia-smi runtime dependencies
  - Width-responsive layout that adapts to narrow terminals

Color palette:
  Cyan       section labels (CPU, RAM, SWAP, GPU, VRAM, temp, refresh:, host:, ...)
  Grey       CPU and GPU brand strings
  Yellow     temperature readings
  Green      bar fill when usage < 50%
  Yellow     bar fill when usage 50-80%
  Red        bar fill when usage >= 80%
  Dark gray  empty bar cells";

const AFTER_LONG_HELP: &str = "\
Keyboard (while running):
  q / Esc    quit
  c          sort by CPU
  r          sort by RAM
  g          sort by GPU
  v          sort by VRAM
  t          sort by total (normalized max)
  Space      pause / resume
  + / -      adjust refresh rate by +/- 0.25 s (clamped 0.25..5.0)

Examples:
  ztop                        Default 1-second refresh, sorted by CPU
  ztop --refresh 0.5          Half-second refresh
  ztop --gpu-peak-hold 5      Keep GPU/VRAM peaks visible for 5 seconds
  ztop --sort total --top 20  Top 20 processes ranked by normalized score
  ztop --no-gpu               Skip NVML; useful on non-NVIDIA hosts

Copyright (c) 2026 Andrea Bodei <info@andreabodei.com>
Dual-licensed MIT OR Apache-2.0";

const VERSION_TEXT: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "\nCopyright (c) 2026 Andrea Bodei <info@andreabodei.com>"
);

#[derive(Parser, Debug)]
#[command(
    name = "ztop",
    version = VERSION_TEXT,
    about = "A simple, GPU-aware Linux system monitor",
    long_about = LONG_ABOUT,
    after_long_help = AFTER_LONG_HELP,
)]
pub struct Cli {
    /// Refresh interval in seconds (0.25..=5.0)
    #[arg(short, long, default_value_t = 1.0, value_parser = parse_refresh)]
    pub refresh: f64,

    /// Initial process sort mode
    #[arg(short, long, value_enum, default_value_t = SortArg::Cpu)]
    pub sort: SortArg,

    /// Number of processes shown in the process table (1..=200)
    #[arg(short = 'n', long, default_value_t = 10, value_parser = parse_top)]
    pub top: usize,

    /// Skip GPU initialization entirely
    #[arg(long)]
    pub no_gpu: bool,

    /// Skip CPU temperature probing
    #[arg(long)]
    pub no_temp: bool,

    /// Rolling GPU peak hold in seconds; 0 disables peak display (0..=30)
    #[arg(long, default_value_t = 3.0, value_parser = parse_gpu_peak_hold, value_name = "SECONDS")]
    pub gpu_peak_hold: f64,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum SortArg {
    Cpu,
    Ram,
    Gpu,
    Vram,
    Total,
}

fn parse_refresh(s: &str) -> Result<f64, String> {
    let v: f64 = s
        .parse()
        .map_err(|e: std::num::ParseFloatError| e.to_string())?;
    if !(0.25..=5.0).contains(&v) {
        return Err(format!("refresh must be in 0.25..=5.0 (got {v})"));
    }
    Ok(v)
}

fn parse_top(s: &str) -> Result<usize, String> {
    let v: usize = s
        .parse()
        .map_err(|e: std::num::ParseIntError| e.to_string())?;
    if !(1..=200).contains(&v) {
        return Err(format!("top must be in 1..=200 (got {v})"));
    }
    Ok(v)
}

fn parse_gpu_peak_hold(s: &str) -> Result<f64, String> {
    let v: f64 = s
        .parse()
        .map_err(|e: std::num::ParseFloatError| e.to_string())?;
    if !(0.0..=30.0).contains(&v) {
        return Err(format!("gpu-peak-hold must be in 0..=30 (got {v})"));
    }
    Ok(v)
}

#[cfg(test)]
mod tests {
    use clap::{error::ErrorKind, Parser};

    use super::{parse_gpu_peak_hold, parse_refresh, parse_top, Cli};

    #[test]
    fn parses_refresh_bounds() {
        assert_eq!(parse_refresh("0.25").unwrap(), 0.25);
        assert_eq!(parse_refresh("5").unwrap(), 5.0);
        assert!(parse_refresh("0.24").is_err());
        assert!(parse_refresh("5.01").is_err());
    }

    #[test]
    fn parses_top_bounds() {
        assert_eq!(parse_top("1").unwrap(), 1);
        assert_eq!(parse_top("200").unwrap(), 200);
        assert!(parse_top("0").is_err());
        assert!(parse_top("201").is_err());
    }

    #[test]
    fn parses_gpu_peak_hold_bounds() {
        assert_eq!(parse_gpu_peak_hold("0").unwrap(), 0.0);
        assert_eq!(parse_gpu_peak_hold("3").unwrap(), 3.0);
        assert_eq!(parse_gpu_peak_hold("30").unwrap(), 30.0);
        assert!(parse_gpu_peak_hold("-0.1").is_err());
        assert!(parse_gpu_peak_hold("30.1").is_err());
    }

    #[test]
    fn version_flag_prints_package_version() {
        let err = Cli::try_parse_from(["ztop", "--version"]).unwrap_err();
        let output = err.to_string();

        assert_eq!(err.kind(), ErrorKind::DisplayVersion);
        assert!(output.contains(env!("CARGO_PKG_VERSION")));
        assert!(output.contains("Copyright (c) 2026 Andrea Bodei <info@andreabodei.com>"));
    }
}
