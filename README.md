# ZTOP

`ZTOP` is a tool to monitor CPU, RAM, GPU, VRAM in the top/htop style and with modern compatibility to nvidia drivers (it works where btop fails). Written from scratch in Rust. `by Andrea Bodei info 'at' andreabodei.com 2026`.

**Status:** v0.3.15 (unreleased dev build). The MVP scope is complete and exceeded — see [`CHANGELOG.md`](./CHANGELOG.md) for the per-version history. Builds and runs against real hardware (NVIDIA NVML + Linux `hwmon`); see [`SPEC.md`](./SPEC.md) for the engineering contract.

A simple, clean, real-time Linux system monitor inspired by `btop`, focused on the essential metrics:

- CPU model and usage
- Per-core CPU usage
- CPU temperature
- RAM usage
- Swap usage
- GPU model, usage and temperature
- VRAM usage
- Top 10 most resource-consuming commands
- User running each command

ZTOP is designed for Linux workstations, GPU servers, AI research machines, cybersecurity labs, and developers who need a fast terminal dashboard without unnecessary complexity.

---

## Goal

ZTOP aims to provide a minimal and readable terminal interface updated every second.

Unlike general-purpose monitors, ZTOP treats GPU and VRAM usage as first-class metrics.

The objective is not to replace `btop`, `htop`, or `nvtop`, but to provide a simpler tool focused on:

```text
CPU + RAM + GPU + VRAM + temperature + top consuming commands
```

---

## Example Interface

Conceptual layout (the actual render adds horizontal bars and a footer with live sort/pause state — see [`SPEC.md`](./SPEC.md#3-layout--width-validation) for the verified 80-column layout):

```text
┌──────────────────────────── ZTOP ────────────────────────────┐
│ Refresh: 1s        Host: workstation-01        Uptime: 04:21:33 │
├─────────────────────────────────────────────────────────────────┤
│ 13th Gen Intel(R) Core(TM) i9-13980HX                          │
│ CPU      42%   Temp: 68°C                                       │
│ Cores    0: 31%  1: 46%  2: 77%  3: 22%                         │
│          4: 55%  5: 39%  6: 81%  7: 18%                         │
├─────────────────────────────────────────────────────────────────┤
│ RAM      18.4 / 32.0 GB    57%                                  │
│ SWAP      0.6 / 31.6 GB     1.8%                                │
├─────────────────────────────────────────────────────────────────┤
│ NVIDIA GeForce RTX 4070 Laptop GPU                              │
│ GPU      73%   Temp: 71°C                                       │
│ VRAM     9.2 / 12.0 GB     76%                                  │
├──────────────────── Top 10 Consuming Commands ──────────────────┤
│ PID     USER       CPU%   RAM%   GPU%   VRAM     COMMAND        │
│ 8123    giaime     188    12.1   65     7.8GB    python train.py │
│ 2411    root       24     4.3    0      0MB      Xorg           │
│ 9931    giaime     18     2.1    4      600MB    blender        │
└─────────────────────────────────────────────────────────────────┘
```

Per-core layout automatically uses the available terminal width, so wide terminals pack more cores per row and high-core-count servers need fewer CPU-panel rows. The process table drops columns as the terminal narrows (VRAM below 76 cols, GPU% below 70, USER below 60).

### Colors

The live UI uses a small palette so the eye can separate labels from values at a glance:

| Style              | Meaning                                                |
| ------------------ | ------------------------------------------------------ |
| **Cyan**           | Section labels (`CPU`, `RAM`, `SWAP`, `GPU`, `VRAM`, `temp`, `Cores`), header field names (`refresh:`, `host:`, `uptime:`), footer keybinds, process-table column headers |
| **Grey**           | Brand strings — CPU model on the first line of the CPU panel, GPU model on the first line of the GPU panel |
| **Yellow**         | Temperature readings                                    |
| **Green / yellow / red** | Bar fill, load-graded: green < 50 %, yellow 50–80 %, red ≥ 80 % |
| **Dark gray**      | Empty bar cells                                         |
| **Default**        | Numeric values, sizes, percentages, PIDs, usernames, command names |

The scheme works on any 256-color or truecolor terminal; on a strictly 16-color terminal the dark-gray empty cells may fall back to plain gray.

---

## Features

### System Metrics

- CPU model (brand string, e.g. `13th Gen Intel(R) Core(TM) i9-13980HX`)
- Total CPU usage
- Per-core CPU usage
- CPU temperature
- RAM used and total RAM
- RAM percentage usage
- Swap used and total
- Swap percentage usage
- System uptime
- Hostname

### GPU Metrics

Initial target: NVIDIA GPUs.

- GPU model (e.g. `NVIDIA GeForce RTX 4070 Laptop GPU`, displayed on the first line of the GPU panel)
- GPU utilization
- GPU temperature
- VRAM used
- VRAM total
- VRAM percentage usage
- GPU process usage where available

Future support:

- AMD GPUs
- Intel GPUs
- Multi-GPU systems

### Process Table

ZTOP displays the top 10 most resource-consuming commands.

Each process row includes:

- PID
- Linux user
- CPU percentage
- RAM percentage
- GPU percentage, where available
- VRAM usage, where available
- Command name

---

## Refresh Rate

Default refresh interval:

```bash
1 second
```

The interface should update continuously without flickering or excessive CPU usage.
GPU metrics are sampled every 250 ms independently from the main UI refresh. The GPU panel keeps a rolling peak for short utilization spikes, defaulting to 3 seconds.

Example:

```bash
ztop --refresh 1
```

---

## Install

The bundled installer bootstraps any missing dependencies, builds in release mode, and copies the binary into `/usr/bin/ztop`:

```bash
./installer.sh
```

If `cargo` or a C toolchain (`cc`, `make`) is missing, the installer fetches them automatically — Rust comes from the official `rustup` one-liner (stable, minimal profile), build essentials come from your distro's package manager (apt / dnf / yum / pacman / zypper / apk are auto-detected). The NVIDIA driver is **not** auto-installed (kernel modules and secure boot make that unsafe from a shell script); the installer warns if it's missing, and ztop will still run with the GPU panel showing `n/a`.

Run the installer as your normal user. `sudo` is invoked only for the package install + final copy steps — the cargo build runs as the current user, so `target/` never ends up root-owned. On hardware where NVML needs elevated privileges for per-process GPU utilization, install normally and run the installed binary with `sudo ztop`.

Preview what would change without touching anything:

```bash
./installer.sh --check
```

Install under a different prefix:

```bash
PREFIX=/usr/local ./installer.sh
```

Remove an installed copy:

```bash
./installer.sh --uninstall
```

---

## Build and Run (manual)

ZTOP is written in Rust. With a stable toolchain (≥ 1.75):

```bash
cargo build --release
./target/release/ztop
```

The binary is ~1.5 MB stripped, statically linkable, and has no runtime dependency on `libsensors` or `nvidia-smi` (NVML is loaded directly).

Some NVIDIA drivers expose per-process GPU utilization only to root or processes with `CAP_SYS_ADMIN`. In that case, build/install as your normal user to keep project files owned by you, then run the installed monitor as root:

```bash
sudo ztop
```

---

## Command-Line Usage

For a concise summary use `-h`; for the full feature tour, color legend, keyboard table, examples, and copyright run `ztop --help`:

```bash
ztop -h        # one-line option summary
ztop --help    # features + colors + keyboard + examples + copyright
```

Other invocations:

```bash
ztop
```

Set refresh interval (0.25–5.0 seconds):

```bash
ztop --refresh 1
```

Set GPU peak hold duration (0–30 seconds, default 3; `0` disables peak display):

```bash
ztop --gpu-peak-hold 5
```

Sort by CPU, RAM, GPU, VRAM, or total:

```bash
ztop --sort cpu
ztop --sort ram
ztop --sort gpu
ztop --sort vram
ztop --sort total
```

Number of processes shown (default 10, range 1–200):

```bash
ztop --top 15
```

Disable GPU monitoring:

```bash
ztop --no-gpu
```

Disable temperature probing:

```bash
ztop --no-temp
```

`--json` is planned for a future release (see [Future Features](#future-features)); v0.1 is terminal-UI only.

---

## Keyboard Bindings

| Key       | Action                                  |
| --------- | --------------------------------------- |
| `q` / Esc | Quit                                    |
| `c`       | Sort by CPU                             |
| `r`       | Sort by RAM                             |
| `g`       | Sort by GPU                             |
| `v`       | Sort by VRAM                            |
| `t`       | Sort by total                           |
| Space     | Pause / resume refresh                  |
| `+` / `-` | Refresh rate ± 0.25 s (clamped 0.25..5) |

---

## Sorting Logic

Default sort: `cpu` (matches `top` and `htop`, least surprise).

The `total` mode uses a *normalized* score so the four components are comparable on a 0..100 scale:

```text
score = max(cpu_pct / n_logical_cpus,  ram_pct,  gpu_pct,  vram_pct)
```

(Per-process CPU% from `sysinfo` is reported as a share of one core and can exceed 100 on multithreaded workloads; dividing by the logical-CPU count yields a device-wide share comparable to RAM/GPU/VRAM percentages.)

Available sorting modes:

```text
cpu     ram     gpu     vram     total
```

---

## Minimum Viable Product

Version `0.1` (implemented, unreleased) covers:

- CPU brand line at the top of the CPU panel
- CPU total usage
- Per-core CPU usage (auto-wraps by terminal width so wide servers fit more cores per row)
- CPU temperature (hwmon-first, thermal_zone fallback)
- RAM used and total RAM
- Swap used and total (panel displays "none configured" when there is no swap)
- NVIDIA GPU name at the top of the GPU panel
- NVIDIA GPU usage
- NVIDIA GPU temperature
- NVIDIA VRAM used and total VRAM
- Per-process VRAM via NVML (per-process GPU% degrades to `—` without root)
- Top 10 processes by resource usage
- Process owner
- Command name
- 1-second refresh (configurable 0.25–5 s)
- Clean terminal UI with horizontal bars and a live footer

---

## Suggested Tech Stack

### Preferred: Rust

Rust is recommended for the first stable implementation because it provides:

- High performance
- Low memory overhead
- Safe system-level programming
- Easy single-binary distribution

Suggested libraries:

```text
ratatui       Terminal UI
crossterm     Terminal input/output
sysinfo       CPU, RAM, and process data
nvml-wrapper  NVIDIA GPU metrics
```

---

## Data Sources

### CPU and RAM

Possible Linux sources:

```text
/proc/stat
/proc/meminfo
/proc/[pid]/stat
/proc/[pid]/status
```

Alternative library:

```text
sysinfo
```

### CPU Temperature

v0.1 reads from sysfs directly — no `libsensors` dependency:

1. Primary: walk `/sys/class/hwmon/*` and prefer entries named `coretemp` (Intel), `k10temp` or `zenpower` (AMD). Pick the input whose label is `Package id 0` or `Tctl`.
2. Fallback: `/sys/class/thermal/thermal_zone*`, preferring zones of type `x86_pkg_temp`.

### NVIDIA GPU

Use NVIDIA Management Library:

```text
NVML
```

NVML can provide:

- GPU utilization
- GPU temperature
- VRAM total
- VRAM used
- GPU process information

### AMD GPU

Possible future sources:

```text
rocm-smi
/sys/class/drm/
```

### Intel GPU

Possible future sources:

```text
intel_gpu_top
/sys/class/drm/
```

---

## Configuration

Future configuration file:

```text
~/.config/ztop/config.toml
```

Example:

```toml
refresh_interval = 1
sort_by = "cpu"
show_gpu = true
show_temperatures = true
top_processes = 10
```

---

## Future Features

Possible features for later versions:

- Multi-GPU support
- AMD GPU support
- Intel GPU support
- JSON output (`--json`)
- Prometheus metrics export
- Logging mode
- Headless mode
- SSH-friendly mode
- Docker/container process detection
- Temperature warnings
- Configurable themes
- Minimal mode
- Detailed process mode
- Per-process GPU utilization without root (currently requires `CAP_SYS_ADMIN` on most distros — see [`SPEC.md` §2.1](./SPEC.md))

---

## Target Users

ZTOP is designed for:

- AI researchers
- CUDA users
- Linux workstation users
- GPU server administrators
- Cybersecurity researchers
- Digital forensic analysts
- Developers
- System administrators

---

## Design Philosophy

ZTOP should be:

### Simple

Only essential information should be shown.

### Clean

The interface should be readable at a glance.

### Fast

The monitor must use minimal CPU and RAM.

### GPU-aware

GPU, VRAM, and GPU temperature should be visible by default.

### User-aware

The process table must clearly show which Linux user is running each command.

---

## Non-Goals

ZTOP should not try to become a full replacement for:

- `btop`
- `htop`
- `top`
- `nvtop`
- `glances`

It should remain focused on a compact and practical resource view.

---

## License

Dual-licensed under either of:

- MIT License
- Apache License 2.0

at your option.

---

## Project Status

Current development version is **0.3.15** (set in `Cargo.toml`; reported by `ztop --version` and by `installer.sh` after install). The v0.1 MVP scope is complete; subsequent iterations have added the swap monitor, CPU/GPU brand lines, a colored palette, merged memory frame, richer help, NVML runtime fallback, lower-overhead event handling, width-aware CPU-core packing, an explicit tested version flag, copyright-bearing version output, active-VRAM accounting aligned with `nvidia-smi`, aligned metric bars, live NVML memory refresh when GPU work starts after `ztop`, rolling GPU peak hold for brief spikes, and an installer step that pulls in `libnvidia-ml-dev` when its package candidate is available so the unversioned NVML soname exists out of the box — see [`CHANGELOG.md`](./CHANGELOG.md) for the full per-version history. Each iteration bumps the version in `Cargo.toml` so the installed binary on `$PATH` always has a verifiable identity distinct from the previous build.

It is **not yet publicly released** — the verification checklist in [`SPEC.md` §7](./SPEC.md) must be completed first (run as root for per-process GPU%, run on a host without NVIDIA, measure idle CPU budget).

Initial platform:

```text
Linux
```

Initial GPU support:

```text
NVIDIA (via NVML)
```

## © 2026 Andrea Bodei <info@andreabodei.com> — Dual-licensed MIT OR Apache-2.0.
