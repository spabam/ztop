// Copyright (c) 2026 Andrea Bodei <info@andreabodei.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

use anyhow::{anyhow, Result};
use std::collections::HashMap;
#[cfg(target_os = "linux")]
use std::ffi::OsStr;
#[cfg(target_os = "linux")]
use std::mem;

#[cfg(target_os = "linux")]
use nvml_wrapper::{enum_wrappers::device::TemperatureSensor, Nvml};
#[cfg(target_os = "linux")]
use nvml_wrapper_sys::bindings::{
    nvmlDevice_t, nvmlMemory_v2_t, nvmlReturn_enum_NVML_SUCCESS, NvmlLib,
};

#[derive(Clone)]
pub struct GpuSnapshot {
    pub name: String,
    pub util_pct: u32,
    pub util_peak_pct: Option<u32>,
    pub temp_c: u32,
    pub vram_used_bytes: u64,
    pub vram_peak_used_bytes: Option<u64>,
    pub vram_total_bytes: u64,
    /// PID -> (gpu_util_pct_or_none, vram_bytes)
    pub per_pid: HashMap<u32, PerPidGpu>,
}

#[derive(Default, Clone, Copy)]
pub struct PerPidGpu {
    pub util_pct: Option<u32>,
    pub vram_bytes: u64,
}

#[cfg(target_os = "linux")]
pub struct GpuProbe {
    nvml: Nvml,
    nvml_raw: Option<NvmlLib>,
    /// device index 0 only for v0.1 (multi-GPU is post-v0.1)
    per_proc_util_supported: bool,
    last_util_query_us: u64,
}

#[cfg(not(target_os = "linux"))]
pub struct GpuProbe;

#[cfg(target_os = "linux")]
impl GpuProbe {
    pub fn init() -> Result<Self> {
        // Default lib name in nvml-wrapper is `libnvidia-ml.so` (unversioned
        // symlink), which ships with the NVIDIA -dev package but not with a
        // runtime-only driver install. On Debian/Ubuntu/Fedora server boxes
        // the runtime ships only the versioned soname `libnvidia-ml.so.1`,
        // so fall back to that when the default open fails.
        let nvml = Nvml::init()
            .or_else(|_| {
                Nvml::builder()
                    .lib_path(OsStr::new("libnvidia-ml.so.1"))
                    .init()
            })
            .map_err(|e| anyhow!("NVML init: {e}"))?;
        let count = nvml
            .device_count()
            .map_err(|e| anyhow!("NVML device_count: {e}"))?;
        if count == 0 {
            return Err(anyhow!("no NVIDIA devices"));
        }
        let nvml_raw = load_nvml_raw();

        // Probe device 0 capabilities. Print init diagnostics to stderr so a
        // user reporting "GPU 0%" can see whether utilization_rates() errored,
        // returned a value, or returned 0 at startup. Output appears in the
        // user's shell history because this runs before EnterAlternateScreen.
        let dev = nvml
            .device_by_index(0)
            .map_err(|e| anyhow!("NVML device_by_index: {e}"))?;
        let name = dev.name().unwrap_or_else(|_| "?".into());
        eprintln!("ztop: NVML init OK; device 0 = \"{name}\" (of {count})");
        match dev.utilization_rates() {
            Ok(u) => eprintln!(
                "ztop:   utilization_rates at init: gpu={}% memory_io={}%",
                u.gpu, u.memory
            ),
            Err(e) => eprintln!("ztop:   utilization_rates FAILED at init: {e}"),
        }
        match memory_info_active(nvml_raw.as_ref()).or_else(|| {
            dev.memory_info().ok().map(|m| ActiveMemoryInfo {
                used: m.used,
                total: m.total,
                reserved: 0,
            })
        }) {
            Some(m) => eprintln!(
                "ztop:   memory_info: used={} MiB, reserved={} MiB, total={} MiB",
                m.used / 1_048_576,
                m.reserved / 1_048_576,
                m.total / 1_048_576
            ),
            None => eprintln!("ztop:   memory_info FAILED"),
        }
        let per_proc_util_supported = dev.process_utilization_stats(None).is_ok();
        if !per_proc_util_supported {
            eprintln!("ztop:   per-process GPU utilization unavailable (driver/permission); column will show '—'");
        }
        Ok(Self {
            nvml,
            nvml_raw,
            per_proc_util_supported,
            last_util_query_us: 0,
        })
    }

    pub fn poll(&mut self) -> Result<GpuSnapshot> {
        let dev = self.nvml.device_by_index(0)?;
        let name = dev.name().unwrap_or_else(|_| "NVIDIA GPU".into());
        let device_util_pct: u32 = dev.utilization_rates().map(|u| u.gpu).unwrap_or(0);
        let mem = dev.memory_info()?;
        let active_mem = memory_info_active(self.nvml_raw.as_ref()).unwrap_or(ActiveMemoryInfo {
            used: mem.used,
            total: mem.total,
            reserved: 0,
        });
        let temp = dev.temperature(TemperatureSensor::Gpu).unwrap_or(0);

        let mut per_pid: HashMap<u32, PerPidGpu> = HashMap::new();

        // Per-process VRAM (compute + graphics)
        if let Ok(procs) = dev.running_compute_processes() {
            for p in procs {
                let entry = per_pid.entry(p.pid).or_default();
                if let nvml_wrapper::enums::device::UsedGpuMemory::Used(bytes) = p.used_gpu_memory {
                    entry.vram_bytes = entry.vram_bytes.max(bytes);
                }
            }
        }
        if let Ok(procs) = dev.running_graphics_processes() {
            for p in procs {
                let entry = per_pid.entry(p.pid).or_default();
                if let nvml_wrapper::enums::device::UsedGpuMemory::Used(bytes) = p.used_gpu_memory {
                    entry.vram_bytes = entry.vram_bytes.max(bytes);
                }
            }
        }

        // Per-process GPU utilization (only if supported)
        if self.per_proc_util_supported {
            if let Ok(stats) = dev.process_utilization_stats(Some(self.last_util_query_us)) {
                for s in &stats {
                    let entry = per_pid.entry(s.pid).or_default();
                    entry.util_pct = Some(s.sm_util);
                }
                if let Some(last) = stats.iter().map(|s| s.timestamp).max() {
                    self.last_util_query_us = last;
                }
            }
        }

        // Fallback: if device-wide utilization came back as 0 — either real,
        // or because the driver returned NOT_SUPPORTED for this device class —
        // take the sum of per-process utilizations as a backstop. Useful when
        // running as root / with CAP_SYS_ADMIN, where process_utilization_stats
        // is populated even if utilization_rates is not.
        let process_util_sum: u32 = per_pid
            .values()
            .filter_map(|e| e.util_pct)
            .sum::<u32>()
            .min(100);
        let util_pct = device_util_pct.max(process_util_sum);

        if std::env::var_os("ZTOP_DEBUG_GPU").is_some() {
            eprintln!(
                "ztop: gpu poll: util={} used={} MiB reserved={} MiB total={} MiB",
                util_pct,
                active_mem.used / 1_048_576,
                active_mem.reserved / 1_048_576,
                active_mem.total / 1_048_576
            );
        }

        Ok(GpuSnapshot {
            name,
            util_pct,
            util_peak_pct: None,
            temp_c: temp,
            vram_used_bytes: active_mem.used,
            vram_peak_used_bytes: None,
            vram_total_bytes: active_mem.total,
            per_pid,
        })
    }
}

#[cfg(target_os = "linux")]
struct ActiveMemoryInfo {
    used: u64,
    total: u64,
    reserved: u64,
}

#[cfg(target_os = "linux")]
fn load_nvml_raw() -> Option<NvmlLib> {
    unsafe {
        let nvml_raw = NvmlLib::new(OsStr::new("libnvidia-ml.so"))
            .or_else(|_| NvmlLib::new(OsStr::new("libnvidia-ml.so.1")))
            .ok()?;

        if nvml_raw.nvmlInit_v2.is_err() || nvml_raw.nvmlInit_v2() != nvmlReturn_enum_NVML_SUCCESS {
            return None;
        }

        Some(nvml_raw)
    }
}

#[cfg(target_os = "linux")]
fn memory_info_active(nvml_raw: Option<&NvmlLib>) -> Option<ActiveMemoryInfo> {
    let nvml_raw = nvml_raw?;
    if nvml_raw.nvmlDeviceGetHandleByIndex_v2.is_err()
        || nvml_raw.nvmlDeviceGetMemoryInfo_v2.is_err()
    {
        return None;
    }

    unsafe {
        let mut device: nvmlDevice_t = mem::zeroed();
        if nvml_raw.nvmlDeviceGetHandleByIndex_v2(0, &mut device) != nvmlReturn_enum_NVML_SUCCESS {
            return None;
        }

        let mut mem_info: nvmlMemory_v2_t = mem::zeroed();
        mem_info.version = nvml_struct_version::<nvmlMemory_v2_t>(2);

        if nvml_raw.nvmlDeviceGetMemoryInfo_v2(device, &mut mem_info)
            != nvmlReturn_enum_NVML_SUCCESS
        {
            return None;
        }

        Some(ActiveMemoryInfo {
            used: mem_info.used,
            total: mem_info.total,
            reserved: mem_info.reserved,
        })
    }
}

#[cfg(target_os = "linux")]
fn nvml_struct_version<T>(version: u32) -> u32 {
    mem::size_of::<T>() as u32 | (version << 24)
}

#[cfg(not(target_os = "linux"))]
impl GpuProbe {
    pub fn init() -> Result<Self> {
        Err(anyhow!("NVML supported on Linux only"))
    }
    pub fn poll(&mut self) -> Result<GpuSnapshot> {
        Err(anyhow!("NVML supported on Linux only"))
    }
}
