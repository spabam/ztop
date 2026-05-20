// Copyright (c) 2026 Andrea Bodei <info@andreabodei.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

use sysinfo::System;

pub fn used_total(sys: &System) -> (u64, u64) {
    (sys.used_memory(), sys.total_memory())
}

pub fn swap_used_total(sys: &System) -> (u64, u64) {
    (sys.used_swap(), sys.total_swap())
}
