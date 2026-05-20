// Copyright (c) 2026 Andrea Bodei <info@andreabodei.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

use sysinfo::System;

pub fn total_usage(sys: &System) -> f32 {
    sys.global_cpu_usage()
}

pub fn per_core_usage(sys: &System) -> Vec<f32> {
    sys.cpus().iter().map(|c| c.cpu_usage()).collect()
}

pub fn logical_count(sys: &System) -> usize {
    sys.cpus().len().max(1)
}

pub fn brand(sys: &System) -> String {
    sys.cpus()
        .first()
        .map(|c| c.brand().trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown CPU".to_string())
}
