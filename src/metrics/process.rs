// Copyright (c) 2026 Andrea Bodei <info@andreabodei.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

use sysinfo::{Process, System, Users};

use crate::app::SortMode;
use crate::metrics::gpu::GpuSnapshot;

pub struct ProcRow {
    pub pid: u32,
    pub user: String,
    pub cpu_pct: f32,
    pub ram_pct: f32,
    pub gpu_pct: Option<u32>,
    pub vram_bytes: u64,
    pub command: String,
}

pub fn collect(
    sys: &System,
    users: &Users,
    gpu: Option<&GpuSnapshot>,
    sort: SortMode,
    n_logical_cpus: usize,
) -> Vec<ProcRow> {
    let total_ram = sys.total_memory().max(1) as f32;

    let mut rows: Vec<ProcRow> = sys
        .processes()
        .iter()
        .map(|(pid, p)| {
            let pid_u32 = pid.as_u32();
            let user = p
                .user_id()
                .and_then(|uid| users.get_user_by_id(uid))
                .map(|u| u.name().to_string())
                .unwrap_or_else(|| "?".into());
            let ram_bytes = p.memory();
            let ram_pct = (ram_bytes as f32 / total_ram) * 100.0;
            let (gpu_pct, vram_bytes) = gpu
                .and_then(|g| g.per_pid.get(&pid_u32))
                .map(|e| (e.util_pct, e.vram_bytes))
                .unwrap_or((None, 0));
            let command = command_line(p);
            ProcRow {
                pid: pid_u32,
                user,
                cpu_pct: p.cpu_usage(),
                ram_pct,
                gpu_pct,
                vram_bytes,
                command,
            }
        })
        .collect();

    sort_rows(&mut rows, sort, n_logical_cpus, gpu);
    rows
}

pub(crate) fn sort_rows(
    rows: &mut [ProcRow],
    sort: SortMode,
    n_cpus: usize,
    gpu: Option<&GpuSnapshot>,
) {
    let vram_total = gpu.map(|g| g.vram_total_bytes.max(1) as f32).unwrap_or(1.0);
    let n_cpus = n_cpus.max(1) as f32;
    rows.sort_by(|a, b| {
        let key = |r: &ProcRow| -> f32 {
            match sort {
                SortMode::Cpu => r.cpu_pct,
                SortMode::Ram => r.ram_pct,
                SortMode::Gpu => r.gpu_pct.unwrap_or(0) as f32,
                SortMode::Vram => r.vram_bytes as f32,
                SortMode::Total => {
                    let cpu_share = r.cpu_pct / n_cpus;
                    let vram_pct = (r.vram_bytes as f32 / vram_total) * 100.0;
                    [
                        cpu_share,
                        r.ram_pct,
                        r.gpu_pct.unwrap_or(0) as f32,
                        vram_pct,
                    ]
                    .into_iter()
                    .fold(0.0_f32, f32::max)
                }
            }
        };
        key(b)
            .partial_cmp(&key(a))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

fn command_line(process: &Process) -> String {
    let cmd = process
        .cmd()
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(" ");

    if cmd.trim().is_empty() {
        process.name().to_string_lossy().to_string()
    } else {
        cmd
    }
}
