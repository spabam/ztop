// Copyright (c) 2026 Andrea Bodei <info@andreabodei.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

mod cpu;
mod gpu;
mod memory;
mod process;
mod temp;

use anyhow::Result;
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};
use sysinfo::{System, Users};

use crate::app::{AppState, SortMode};
use crate::cli::Cli;

pub use gpu::GpuSnapshot;
pub use process::ProcRow;

pub struct Snapshot {
    pub hostname: String,
    pub uptime_secs: u64,
    pub cpu_brand: String,
    pub cpu_total_pct: f32,
    pub cpu_per_core_pct: Vec<f32>,
    pub cpu_temp_c: Option<f32>,
    pub ram_used_bytes: u64,
    pub ram_total_bytes: u64,
    pub swap_used_bytes: u64,
    pub swap_total_bytes: u64,
    pub gpu: Option<GpuSnapshot>,
    pub procs: Vec<ProcRow>,
}

impl Snapshot {
    pub fn sort_processes(&mut self, sort: SortMode) {
        process::sort_rows(
            &mut self.procs,
            sort,
            self.cpu_per_core_pct.len().max(1),
            self.gpu.as_ref(),
        );
    }
}

pub struct Collector {
    sys: System,
    users: Users,
    last_user_refresh: Instant,
    gpu: Option<gpu::GpuProbe>,
    gpu_snapshot: Option<GpuSnapshot>,
    gpu_peaks: GpuPeakHistory,
    gpu_sample_interval: Duration,
    last_gpu_sample: Option<Instant>,
    temp: Option<temp::CpuTempReader>,
}

impl Collector {
    pub fn new(cli: &Cli) -> Result<Self> {
        let mut sys = System::new_all();
        sys.refresh_all();

        let gpu = if cli.no_gpu {
            None
        } else {
            match gpu::GpuProbe::init() {
                Ok(probe) => Some(probe),
                Err(e) => {
                    eprintln!("ztop: GPU init failed ({e}); continuing without GPU metrics");
                    None
                }
            }
        };

        Ok(Self {
            sys,
            users: Users::new_with_refreshed_list(),
            last_user_refresh: Instant::now(),
            gpu,
            gpu_snapshot: None,
            gpu_peaks: GpuPeakHistory::new(Duration::from_millis(
                (cli.gpu_peak_hold * 1000.0).round() as u64,
            )),
            gpu_sample_interval: Duration::from_millis(250),
            last_gpu_sample: None,
            temp: (!cli.no_temp).then(temp::CpuTempReader::new),
        })
    }

    pub fn sample_gpu(&mut self) {
        let Some(gpu) = self.gpu.as_mut() else {
            return;
        };

        let result = gpu.poll();
        let sampled_at = Instant::now();
        self.last_gpu_sample = Some(sampled_at);

        match result {
            Ok(mut snap) => {
                self.gpu_peaks.apply(sampled_at, &mut snap);
                self.gpu_snapshot = Some(snap);
            }
            Err(_) => {
                self.gpu_snapshot = None;
            }
        }
    }

    pub fn sample_gpu_if_due(&mut self, now: Instant) {
        if self.gpu_sample_wait(now) == Some(Duration::ZERO) {
            self.sample_gpu();
        }
    }

    pub fn gpu_sample_wait(&self, now: Instant) -> Option<Duration> {
        self.gpu.as_ref()?;
        Some(match self.last_gpu_sample {
            Some(last) => self
                .gpu_sample_interval
                .saturating_sub(now.duration_since(last)),
            None => Duration::ZERO,
        })
    }

    pub fn collect(&mut self, app: &AppState) -> Snapshot {
        if self.last_user_refresh.elapsed() >= Duration::from_secs(60) {
            self.users.refresh_list();
            self.last_user_refresh = Instant::now();
        }

        self.sys.refresh_cpu_all();
        self.sys.refresh_memory();
        self.sys
            .refresh_processes(sysinfo::ProcessesToUpdate::All, true);

        let cpu_brand = cpu::brand(&self.sys);
        let cpu_total_pct = cpu::total_usage(&self.sys);
        let cpu_per_core_pct = cpu::per_core_usage(&self.sys);
        let cpu_temp_c = self.temp.as_mut().and_then(temp::CpuTempReader::read);

        let (ram_used_bytes, ram_total_bytes) = memory::used_total(&self.sys);
        let (swap_used_bytes, swap_total_bytes) = memory::swap_used_total(&self.sys);

        let gpu_snap = self.gpu_snapshot.clone();

        let procs = process::collect(
            &self.sys,
            &self.users,
            gpu_snap.as_ref(),
            app.sort,
            cpu::logical_count(&self.sys),
        );

        Snapshot {
            hostname: sysinfo::System::host_name().unwrap_or_else(|| "unknown".into()),
            uptime_secs: sysinfo::System::uptime(),
            cpu_brand,
            cpu_total_pct,
            cpu_per_core_pct,
            cpu_temp_c,
            ram_used_bytes,
            ram_total_bytes,
            swap_used_bytes,
            swap_total_bytes,
            gpu: gpu_snap,
            procs,
        }
    }
}

struct GpuPeakHistory {
    hold: Duration,
    samples: VecDeque<GpuPeakSample>,
}

struct GpuPeakSample {
    at: Instant,
    util_pct: u32,
    vram_used_bytes: u64,
}

impl GpuPeakHistory {
    fn new(hold: Duration) -> Self {
        Self {
            hold,
            samples: VecDeque::new(),
        }
    }

    fn apply(&mut self, now: Instant, snap: &mut GpuSnapshot) {
        if self.hold.is_zero() {
            self.samples.clear();
            snap.util_peak_pct = None;
            snap.vram_peak_used_bytes = None;
            return;
        }

        self.samples.push_back(GpuPeakSample {
            at: now,
            util_pct: snap.util_pct,
            vram_used_bytes: snap.vram_used_bytes,
        });

        while self
            .samples
            .front()
            .is_some_and(|sample| now.duration_since(sample.at) > self.hold)
        {
            self.samples.pop_front();
        }

        snap.util_peak_pct = self.samples.iter().map(|sample| sample.util_pct).max();
        snap.vram_peak_used_bytes = self
            .samples
            .iter()
            .map(|sample| sample.vram_used_bytes)
            .max();
    }
}

#[cfg(test)]
mod tests {
    use super::{GpuPeakHistory, GpuSnapshot};
    use std::{collections::HashMap, time::Duration};

    #[test]
    fn gpu_peak_history_holds_recent_maxima() {
        let mut peaks = GpuPeakHistory::new(Duration::from_secs(3));
        let start = std::time::Instant::now();

        let mut first = gpu_snapshot(10, 100);
        peaks.apply(start, &mut first);
        assert_eq!(first.util_peak_pct, Some(10));
        assert_eq!(first.vram_peak_used_bytes, Some(100));

        let mut spike = gpu_snapshot(96, 1_000);
        peaks.apply(start + Duration::from_secs(1), &mut spike);
        assert_eq!(spike.util_peak_pct, Some(96));
        assert_eq!(spike.vram_peak_used_bytes, Some(1_000));

        let mut after = gpu_snapshot(0, 200);
        peaks.apply(start + Duration::from_secs(2), &mut after);
        assert_eq!(after.util_peak_pct, Some(96));
        assert_eq!(after.vram_peak_used_bytes, Some(1_000));

        let mut expired = gpu_snapshot(5, 150);
        peaks.apply(start + Duration::from_secs(6), &mut expired);
        assert_eq!(expired.util_peak_pct, Some(5));
        assert_eq!(expired.vram_peak_used_bytes, Some(150));
    }

    #[test]
    fn gpu_peak_history_can_be_disabled() {
        let mut peaks = GpuPeakHistory::new(Duration::ZERO);
        let mut snap = gpu_snapshot(96, 1_000);

        peaks.apply(std::time::Instant::now(), &mut snap);

        assert_eq!(snap.util_peak_pct, None);
        assert_eq!(snap.vram_peak_used_bytes, None);
    }

    fn gpu_snapshot(util_pct: u32, vram_used_bytes: u64) -> GpuSnapshot {
        GpuSnapshot {
            name: "GPU".into(),
            util_pct,
            util_peak_pct: None,
            temp_c: 0,
            vram_used_bytes,
            vram_peak_used_bytes: None,
            vram_total_bytes: 2_000,
            per_pid: HashMap::new(),
        }
    }
}
