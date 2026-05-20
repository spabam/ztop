// Copyright (c) 2026 Andrea Bodei <info@andreabodei.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fs;
use std::path::{Path, PathBuf};

pub struct CpuTempReader {
    source: Option<PathBuf>,
}

impl CpuTempReader {
    /// Cache the selected CPU package temperature path.
    ///
    /// Strategy (per SPEC §2.3):
    ///   1. Walk /sys/class/hwmon/*, prefer name = coretemp | k10temp | zenpower.
    ///   2. Within chosen hwmon, prefer tempN_label matching "Package id 0" or "Tctl",
    ///      else temp1_input.
    ///   3. Fallback: /sys/class/thermal/thermal_zone* with type "x86_pkg_temp",
    ///      else first available zone.
    pub fn new() -> Self {
        Self {
            source: find_cpu_temp_path(),
        }
    }

    pub fn read(&mut self) -> Option<f32> {
        if let Some(path) = &self.source {
            if let Some(temp) = read_temp_input(path) {
                return Some(temp);
            }
        }

        self.source = find_cpu_temp_path();
        self.source.as_deref().and_then(read_temp_input)
    }
}

fn find_cpu_temp_path() -> Option<PathBuf> {
    find_hwmon_temp_path().or_else(find_thermal_zone_temp_path)
}

fn find_hwmon_temp_path() -> Option<PathBuf> {
    const PREFERRED: &[&str] = &["coretemp", "k10temp", "zenpower"];
    let entries = fs::read_dir("/sys/class/hwmon").ok()?;

    let mut candidates: Vec<(usize, PathBuf, String)> = Vec::new();
    for ent in entries.flatten() {
        let path = ent.path();
        let name = fs::read_to_string(path.join("name"))
            .ok()
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        if name.is_empty() {
            continue;
        }
        let rank = PREFERRED
            .iter()
            .position(|p| *p == name)
            .unwrap_or(usize::MAX);
        candidates.push((rank, path, name));
    }
    candidates.sort_by_key(|c| c.0);

    for (rank, dir, _name) in candidates {
        if rank == usize::MAX {
            continue; // only use known CPU sensors at this stage
        }
        if let Some(path) = labeled_temp_path(&dir) {
            return Some(path);
        }
        let path = dir.join("temp1_input");
        if path.exists() {
            return Some(path);
        }
    }
    None
}

fn labeled_temp_path(dir: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    for ent in entries.flatten() {
        let p = ent.path();
        let fname = p.file_name()?.to_str()?.to_string();
        if !fname.ends_with("_label") {
            continue;
        }
        let label = fs::read_to_string(&p).ok()?.trim().to_string();
        if label == "Package id 0" || label == "Tctl" || label == "Tdie" {
            let input = dir.join(fname.replace("_label", "_input"));
            if input.exists() {
                return Some(input);
            }
        }
    }
    None
}

fn read_temp_input(path: &Path) -> Option<f32> {
    let s = fs::read_to_string(path).ok()?;
    let v: i32 = s.trim().parse().ok()?;
    Some(v as f32 / 1000.0)
}

fn find_thermal_zone_temp_path() -> Option<PathBuf> {
    let entries = fs::read_dir("/sys/class/thermal").ok()?;
    let mut zones: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("thermal_zone"))
                .unwrap_or(false)
        })
        .collect();
    zones.sort();

    // Prefer x86_pkg_temp
    for z in &zones {
        let ty = fs::read_to_string(z.join("type"))
            .ok()
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        if ty == "x86_pkg_temp" {
            let path = z.join("temp");
            if path.exists() {
                return Some(path);
            }
        }
    }
    // Fallback to first readable zone
    for z in &zones {
        let path = z.join("temp");
        if path.exists() {
            return Some(path);
        }
    }
    None
}
