# Changelog

All notable changes to ZTOP are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

> **Pre-release policy:** ZTOP is unreleased; every change set during development bumps the version in `Cargo.toml`. Numbers are not yet load-bearing public commitments — they exist so each rebuild has a verifiable, distinct identity. `./installer.sh` reports the version after install so the user can confirm the binary on `$PATH` matches the source tree.

---

## [Unreleased]

(nothing yet)

## [0.3.15] — 2026-05-20 — Installer NVML symlink helper

### Changed

- `installer.sh` now checks for the unversioned `libnvidia-ml.so` symlink after detecting an NVIDIA driver. When it is missing on apt-based systems and `libnvidia-ml-dev` is a candidate in the configured repos (Ubuntu, NVIDIA CUDA repo on Debian), the installer offers to install it; otherwise it logs that ztop will use its built-in `libnvidia-ml.so.1` fallback. This stops the `libnvidia-ml.so: cannot open shared object file` failure on stock Debian Trixie hosts where only the versioned soname ships in `libnvidia-ml1`.

## [0.3.14] — 2026-05-20 — GPU peak hold

### Added

- GPU sampling now runs every 250 ms independently from the main UI refresh, so short NVML utilization events can be captured without refreshing the full process table four times per second.
- The GPU panel now shows a rolling utilization peak, defaulting to a 3-second hold window, so brief model-load spikes remain visible on the next UI refresh.
- Added `--gpu-peak-hold SECONDS` with range `0..=30`; `0` disables peak tracking and hides the peak values.
- The GPU title line also reports rolling peak VRAM while peak tracking is enabled.

## [0.3.13] — 2026-05-20 — Live NVML v2 refresh

### Fixed

- The raw NVML memory-info v2 handle now initializes itself with `nvmlInit_v2()`, so active VRAM readings update correctly when an Ollama model is loaded after `ztop` is already running.

## [0.3.12] — 2026-05-20 — Aligned metric bars

### Fixed

- CPU and GPU utilization bars now start at the same column as the RAM, SWAP, and VRAM bars, matching the tighter visual alignment used by tools like `btop`.
- Metric lines now use one shared bar-start column helper instead of separate per-line subtraction constants.

## [0.3.11] — 2026-05-20 — NVIDIA active VRAM accounting

### Fixed

- VRAM used now prefers NVML memory-info v2, matching `nvidia-smi memory.used` by excluding driver/firmware reserved memory from the active-used total.
- Startup GPU diagnostics now print reserved memory alongside used and total memory when NVML memory-info v2 is available.

## [0.3.10] — 2026-05-20 — Copyrighted version output

### Changed

- `ztop --version` now prints the package version followed by `Copyright (c) 2026 Andrea Bodei <info@andreabodei.com>`.
- Expanded the version-flag regression test to assert that the copyright line is present.

## [0.3.9] — 2026-05-20 — Explicit version flag

### Added

- `ztop --version` is now explicitly wired to `CARGO_PKG_VERSION` in the CLI definition and covered by a regression test.

## [0.3.8] — 2026-05-20 — Wide CPU-core packing

### Fixed

- Per-core CPU rows now use the available terminal width instead of always stopping at 8 cores per row. This reduces the CPU panel height on wide terminals, so high-core-count servers such as 128-logical-core hosts can show all cores instead of clipping the lower rows.
- Core IDs now reserve enough width for three-digit indices, keeping `100` through `127` aligned in the CPU panel.

## [0.3.7] — 2026-05-20 — Runtime hygiene and ownership-safe install

### Added

- Validated `--top` range (`1..=200`) and added focused unit tests for CLI bounds and refresh-rate clamping.
- `LICENSE-MIT` and `LICENSE-APACHE` files for the dual-license declaration.
- GitHub Actions CI workflow for `cargo fmt --check`, `cargo clippy -- -D warnings`, and `cargo test --locked`.

### Changed

- The TUI event loop now redraws only after input or refresh ticks, reducing idle rendering work.
- Sort hotkeys immediately re-sort the current process snapshot, including while paused.
- Process rows now prefer the full command line over the executable name when `/proc` exposes it.
- User lookup and CPU temperature source discovery are cached instead of rebuilt on every tick.
- Terminal cleanup now uses a drop guard, and release builds use unwinding panics so the guard can restore the terminal if a panic occurs.
- `installer.sh` now refuses to build as root; install as a normal user, then run `sudo ztop` on hardware that needs root for per-process GPU metrics.
- `Cargo.lock` is no longer ignored; this is a binary application and should keep reproducible dependency resolution.

### Removed

- Removed the stale `README.md~` backup file from the project.

## [0.3.6] — 2026-05-20 — Installer NVML symlink bootstrap

The 0.3.4 binary already falls back from `libnvidia-ml.so` (unversioned, dev-package symlink) to `libnvidia-ml.so.1` (runtime soname) at NVML load time. This release adds the *complementary* installer-side fix: when NVIDIA is detected and the unversioned symlink is missing, the installer tries to install it via the distro's package manager so the canonical default-name open path works without needing the binary's fallback.

### Added

- `installer.sh::check_nvidia` is now two-phase:
  1. Detect NVIDIA presence via `nvidia-smi` or any of the standard `libnvidia-ml.so.1` locations.
  2. If NVIDIA is present, check whether the unversioned `libnvidia-ml.so` symlink exists in `/usr/lib/x86_64-linux-gnu`, `/usr/lib64`, `/usr/lib`, `/lib`, or `/lib64`. If it does not, attempt to install the package that provides it.
- Per-package-manager handling for the unversioned symlink:
  - **apt** — runs `apt-cache policy libnvidia-ml-dev` first and only attempts the install when there's a valid candidate. This avoids `E: no installation candidate` on stock Debian (where the symlink ships only via the NVIDIA CUDA repo as `cuda-nvml-dev-XX-X`, not in main).
  - **dnf / yum** — warns that the symlink is missing and points at `cuda-nvml-devel-*` from the CUDA repo as the canonical source.
  - **other / unknown** — warns and relies on the binary's `.so.1` fallback.
- Refactored NVIDIA detection so the symlink check only runs when a driver is actually present (no point installing `-dev` headers on a host that doesn't have the driver itself).

### Notes

- The binary's `.so.1` fallback from 0.3.4 remains the safety net: if `libnvidia-ml-dev` can't be installed (no candidate, package manager unsupported, install fails), `ztop` still initializes NVML by opening `libnvidia-ml.so.1` directly. The installer prints `ztop will use the .so.1 fallback` in those paths so the user knows what happened.
- No source-code change in this release — only `installer.sh` and the version field in `Cargo.toml`.

## [0.3.5] — 2026-05-20 — GPU utilization diagnostics and fallback

User report: "GPU usage is always 0% even when using ollama and llms" on multiple hosts. Adding visibility into what NVML is actually returning at startup, plus a per-process utilization fallback for the case where `nvmlDeviceGetUtilizationRates` returns NOT_SUPPORTED or 0 while ollama is producing real work.

### Added

- **Init-time NVML diagnostic dump to stderr** (one-shot, before the TUI's alt-screen takes over so the lines stay in the user's shell history). Prints device name and count, the result of `utilization_rates()` at init (success with values, or the actual error message), the result of `memory_info()`, and whether per-process utilization is permitted. This lets a "GPU 0%" report be triaged: did the API error, return 0, or return a real value?
- **Process-util-sum fallback** in `GpuProbe::poll()`. When `utilization_rates()` returns 0 — either because it failed (suppressed) or because the driver reports NOT_SUPPORTED for the device class — the device-wide util_pct now reports `max(device_util, sum(per_process_sm_util))`. Only effective when per-process utilization is available (root or `CAP_SYS_ADMIN`); harmless to non-root sessions because the per_pid util_pct values are all `None` and the sum is 0.

### Notes for triage

If the init line reads `utilization_rates at init: gpu=0% memory_io=0%`, NVML is responding but the driver is reporting 0 utilization for the device — likely a sampling-window or power-state issue, or the GPU is genuinely idle at startup. Run `nvidia-smi dmon -s u` alongside ztop to compare.

If the init line reads `utilization_rates FAILED at init: <error>`, the API is not supported on the device or driver. The fallback will populate util from per-process stats when running as root.

## [0.3.4] — 2026-05-20 — NVML and installer robustness

### Fixed

- **NVML init failed on servers without the `nvidia-utils-*-dev` package.** `nvml-wrapper` defaults to opening `libnvidia-ml.so` (the unversioned symlink), which only ships with the `-dev` package. On runtime-only installs (Debian/Ubuntu/Fedora server images, the user's `hypervisor00` with NVIDIA A2), only the versioned soname `libnvidia-ml.so.1` is present, and ztop reported `libnvidia-ml.so: cannot open shared object file` even with a working driver. `metrics::gpu::GpuProbe::init()` now tries the default open and falls back to `libnvidia-ml.so.1` via `Nvml::builder().lib_path(...)`.
- **Installer reinstalled rustup on every run** in shells that hadn't sourced `~/.cargo/env`. `command -v cargo` only checks the current shell's `PATH`; rustup writes the PATH update into `.profile` / `.bashrc` which non-login non-interactive shells don't always source. `installer.sh::ensure_rust` now sources `~/.cargo/env` before the `command -v` check, so a previously-installed toolchain is detected without redoing the curl-pipe-to-sh.

## [0.3.3] — 2026-05-20 — Rich --help

### Added

- `ztop --help` now prints a full feature tour, color palette legend, keyboard bindings table, usage examples, and the copyright/license line. The short `-h` form keeps the concise one-line summary; `--help` triggers the long form via clap's `long_about` + `after_long_help`.
- Per-flag doc comments in `src/cli.rs` so each option has an inline description in the `Options:` block of `--help` (e.g. `--refresh` now shows "Refresh interval in seconds (0.25..=5.0)" instead of being undocumented).
- Copyright line in `--help` output: `Copyright (c) 2026 Andrea Bodei <info@andreabodei.com>` followed by the dual MIT/Apache-2.0 declaration.

### Changed

- `src/cli.rs` reorganized with `LONG_ABOUT` and `AFTER_LONG_HELP` `const &str` blocks at the top of the file, so the help text is co-located with the CLI definition and trivially editable.

## [0.3.2] — 2026-05-20 — Installer bootstrap

### Added

- `installer.sh` now **bootstraps its own dependencies**:
  - If `cargo` is missing, installs the Rust stable toolchain via the official rustup one-liner (`--profile minimal`, non-interactive). If `curl` is missing it is installed first via the detected package manager.
  - If `cc` or `make` is missing, installs build essentials (`build-essential` + `pkg-config` on Debian/Ubuntu; `gcc make pkg-config` on Fedora/RHEL; `base-devel` on Arch; `devel_basis` pattern on openSUSE; `build-base` on Alpine).
  - Detects NVIDIA driver presence (`nvidia-smi` or `libnvidia-ml.so.1`) and warns if missing — the driver itself is **not** auto-installed because it requires kernel modules and is entangled with secure boot.
- New `--check` subcommand: prints the environment audit (rust, build tools, NVIDIA, installed binary) without making any changes.
- `pkg_install` helper that abstracts over apt / dnf / yum / pacman / zypper / apk.

### Changed

- Installer help text expanded to document the new subcommands and bootstrap semantics.

## [0.3.1] — 2026-05-20

### Changed

- **RAM and SWAP merged into a single bordered panel** (height 4: 2 content lines + 2 borders). Previously each had its own frame stacked vertically; sharing one frame reduces visual noise without changing the information content.
- `draw_ram()` and `draw_swap()` collapsed into one `draw_memory()` that emits both lines into the same `Paragraph`.
- Vertical layout shrank from seven chunks back to six (header, cpu, memory, gpu, processes, footer).

## [0.3.0] — 2026-05-20 — Color palette

### Added

- **Cyan** for section labels (`CPU`, `RAM`, `SWAP`, `GPU`, `VRAM`, `temp`, `Cores`), header field names (`refresh:`, `host:`, `uptime:`), footer keybinds, and process-table column headers.
- **Grey** for brand strings (CPU model line, GPU model line) — chosen for a quieter look that lets the cyan labels and yellow temperatures carry the visual emphasis. (Bold magenta was tried first and explicitly downgraded.)
- **Yellow** for temperature readings.
- **Load-graded bar fills**: green when usage < 50 %, yellow at 50–80 %, red ≥ 80 %.
- **Dark gray** for empty bar cells, clearly separating filled from unfilled portions.
- `ui::bars::bar_spans()` replaces the old plain-text `bar()`; returns a `Vec<Span<'static>>` so each bar can be styled per character class (fill vs empty vs brackets) within a single line.
- Compile-time `const Style` palette entries (`LABEL`, `BRAND`, `TEMP`, `TITLE`) at the top of `src/ui/mod.rs` so the colors are declared in one place and trivially tweakable.

### Changed

- All panel labels now produced as individual styled spans instead of being baked into one raw format string. Visible character counts preserved exactly so the resize breakpoints in SPEC §3 are unaffected.
- Process table header row now bold cyan instead of bold default.

## [0.2.1] — 2026-05-20 — Licensing & distribution

### Added

- SPDX-style copyright headers on every source, config, and doc file: `Copyright (c) 2026 Andrea Bodei <info@andreabodei.com>` + `SPDX-License-Identifier: MIT OR Apache-2.0`.
- `authors = ["Andrea Bodei <info@andreabodei.com>"]` in `Cargo.toml`.
- `installer.sh` for `/usr/bin/ztop` install (with `PREFIX` override and `--uninstall` subcommand). Sudo is used only for the copy step so `target/` never ends up root-owned.

### Changed

- License declaration: "to be decided" → dual MIT OR Apache-2.0.
- README rewritten to reflect the implemented state, with `Install`, `Build and Run (manual)`, and `Keyboard Bindings` sections.

## [0.2.0] — 2026-05-20 — Swap monitor and brand lines

### Added

- **Swap memory panel** between RAM and GPU. Used / total GB and percentage with a horizontal bar. When no swap device is configured, the panel displays `SWAP  none configured` instead of a divide-by-zero placeholder.
- **CPU brand line** at the top of the CPU panel, mirroring the GPU panel's device-name treatment. Sourced from `sysinfo::Cpu::brand()` (e.g. `13th Gen Intel(R) Core(TM) i9-13980HX`).
- `metrics::memory::swap_used_total()` helper.
- `metrics::cpu::brand()` helper with empty-string and missing-CPU fallback to `"unknown CPU"`.
- `ui::mod::truncate()` shared helper for narrow-terminal brand-line trimming.

### Changed

- **GPU panel device name moved from the third line to the first.** New order: GPU model → GPU util + temp + bar → VRAM used/total + bar. The relocation matches the new CPU panel layout.
- CPU panel height grew by one line to accommodate the brand line (`ceil(n_cores / 8) + 4`).
- `Snapshot` gained `cpu_brand`, `swap_used_bytes`, `swap_total_bytes` fields.
- Vertical layout grew from six chunks to seven (header, cpu, ram, swap, gpu, processes, footer).

## [0.1.0] — 2026-05-20 — Initial scaffold

The inaugural development cycle: project goes from a README-only concept to a working, end-to-end Linux/NVIDIA monitor that runs against real hardware.

### Added

#### Engineering contract

- `SPEC.md` resolving the five open design questions raised in the concept review:
  - per-process GPU/VRAM graceful degradation when `nvmlDeviceGetProcessUtilization` is not permitted,
  - normalized `total` score so CPU%/RAM%/GPU%/VRAM% are comparable on 0..100,
  - hwmon-based CPU temperature with thermal_zone fallback,
  - explicit keyboard bindings,
  - `--json` deferred (also removed from the duplicated CLI listing).
- 80-column layout validation with per-column byte budgets and shrinkage breakpoints (76 / 70 / 60 / 50 / 40 cols).
- Pre-release verification checklist (SPEC §7).

#### Rust scaffold

- `Cargo.toml` with `ratatui` 0.29, `crossterm` 0.28, `sysinfo` 0.32, `nvml-wrapper` 0.10 (Linux-only), `clap` 4, `anyhow` 1; release profile with thin LTO and strip.
- Module layout under `src/`:
  - `main.rs` — terminal setup/teardown, event loop, key dispatch.
  - `cli.rs` — `clap` definition with validated `--refresh` range.
  - `app.rs` — `AppState`, `SortMode`, refresh rate clamping.
  - `metrics/mod.rs` — `Collector`, `Snapshot`.
  - `metrics/cpu.rs`, `metrics/memory.rs` — sysinfo wrappers.
  - `metrics/temp.rs` — `/sys/class/hwmon` scanner with thermal_zone fallback.
  - `metrics/gpu.rs` — NVML probe; per-PID VRAM merge from compute + graphics processes; per-PID GPU% when permitted.
  - `metrics/process.rs` — sysinfo + NVML join, sort by selected mode, truncate to top-N.
  - `ui/mod.rs` — vertical layout, panel renderers, width-based column drops.
  - `ui/bars.rs` — horizontal bar widget.
  - `ui/table.rs` — process table with adaptive columns.

#### Runtime features

- CLI flags: `--refresh` (0.25..5.0), `--sort {cpu|ram|gpu|vram|total}`, `--top N`, `--no-gpu`, `--no-temp`.
- Keyboard bindings: `q`/Esc quit, `c`/`r`/`g`/`v`/`t` sort, Space pause, `+`/`-` adjust refresh by 0.25 s.
- Live footer showing active sort and pause state.
- Width-responsive process table that drops `VRAM` < 76 cols, `GPU%` < 70, `USER` < 60, and refuses to render below 40.
- One-line stderr notice when per-process GPU utilization is unavailable (driver/permission), with `—` in that column.

### Changed (vs original README concept)

- Default sort: `total` → `cpu` (matches `top` and `htop`, least surprise).
- `total` score formula: `cpu% + ram% + gpu% + vram%` → `max(cpu% / n_logical_cpus, ram%, gpu%, vram%)` so per-process CPU% becomes comparable with the other 0..100 components.
- CPU temperature primary source: `/sys/class/thermal` → `/sys/class/hwmon` scan preferring `coretemp`/`k10temp`/`zenpower`.

### Fixed

- Per-core panel was hard-coded to 4 lines; on a 32-core machine only the first 4 cores rendered. Panel height is now `ceil(n_cores / 8) + 3` and cores pack 8 per row.
- Bar bracket clipping on RAM and VRAM lines — the width math omitted the 2-character block borders.

### Removed

- `--json` CLI flag (deferred to a later release alongside Prometheus export, when an output schema is defined).

---

## Still outstanding before the first public release

- Run as root and confirm per-process GPU% column populates.
- Run on a host without an NVIDIA GPU and confirm the GPU panel shows `n/a` without panicking.
- Measure idle CPU budget rigorously (`perf stat`) and confirm < 1 % on an 8-core box at default refresh.
- Walk every resize breakpoint visually with a live terminal.

---

© 2026 Andrea Bodei <info@andreabodei.com> — Dual-licensed MIT OR Apache-2.0.
