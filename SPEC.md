# ZTOP v0.1 — Tightened Spec

This document refines `README.md`, resolving the open design questions raised in review before implementation begins. The `README.md` remains the public-facing concept; this is the engineering contract for the first release.

---

## 1. v0.1 Scope (unchanged from README MVP)

- CPU total %, per-core %, package temperature
- RAM used / total / %
- NVIDIA GPU util %, temperature, VRAM used / total / %
- Top-N process table with PID, user, CPU%, RAM%, GPU%, VRAM, command
- 1 s default refresh, configurable
- Single static binary

Out of scope for v0.1: AMD/Intel GPUs, multi-GPU, JSON / Prometheus export, themes, containers, headless mode, config file.

---

## 2. Open questions resolved

### 2.1 Per-process GPU / VRAM (graceful degradation)

Two NVML calls back this column:

| Field             | NVML call                                                                            | Privilege            |
| ----------------- | ------------------------------------------------------------------------------------ | -------------------- |
| Per-process VRAM  | `nvmlDeviceGetComputeRunningProcesses_v3` + `nvmlDeviceGetGraphicsRunningProcesses_v3` | Unprivileged         |
| Per-process GPU % | `nvmlDeviceGetProcessUtilization`                                                    | Driver ≥ 410; some distros need `CAP_SYS_ADMIN` or root |

**Strategy:** probe once at startup. If per-process GPU % is unavailable, render `—` in that column and log a one-line notice on stderr when stderr is a tty. Aggregate (device-wide) GPU % is always shown — it does not require those calls.

### 2.2 Default sort

- Default changes from `total` → **`cpu`** (least surprise; matches `top`).
- `total` is retained but redefined so all four components share a 0..100 scale:

  ```text
  score = max(cpu_pct / n_logical_cpus,  ram_pct,  gpu_pct,  vram_pct)
  ```

  (sysinfo reports per-process CPU% as a share of *one* core, so a multithreaded process can exceed 100; dividing by `n_logical_cpus` yields a device-wide share comparable to RAM/GPU/VRAM percentages.)

### 2.3 CPU temperature source

Order of attempts:

1. Scan `/sys/class/hwmon/*/name`. Prefer entries named `coretemp` (Intel), `k10temp` / `zenpower` (AMD).
2. Within that hwmon, pick the `tempN_input` whose sibling `tempN_label` is `Package id 0` (Intel) or `Tctl` (AMD). If no label matches, take `temp1_input`.
3. Fallback: `/sys/class/thermal/thermal_zone*` with type `x86_pkg_temp`, else first available zone.
4. Read value is millidegrees C — divide by 1000.

No `libsensors` dependency for v0.1. This keeps the build pure-Rust and the binary statically linkable.

### 2.4 Keyboard interaction (new — README was CLI-only)

| Key       | Action                                |
| --------- | ------------------------------------- |
| `q` / Esc | Quit                                  |
| `c`       | Sort by CPU                           |
| `r`       | Sort by RAM                           |
| `g`       | Sort by GPU                           |
| `v`       | Sort by VRAM                          |
| `t`       | Sort by total (normalized score)      |
| Space     | Pause / resume refresh                |
| `+` / `-` | Refresh rate ± 0.25 s (clamped 0.25..5) |

A footer line shows the active sort and pause state.

### 2.5 `--json` deferred

Removed from v0.1 CLI. The README listed it twice (current flag + future feature) — that ambiguity is resolved by deferring it. It will return alongside Prometheus export once an output schema is defined.

---

## 3. Layout — width validation

Target: 80 columns (POSIX default). Mockup re-laid against an 80-col grid:

```text
0         1         2         3         4         5         6         7         8
0123456789012345678901234567890123456789012345678901234567890123456789012345678901
┌──────────────────────────────── ZTOP ────────────────────────────────────────┐
│ Refresh: 1.0s     Host: workstation-01            Uptime: 04:21:33           │
├──────────────────────────────────────────────────────────────────────────────┤
│ CPU   42%  Temp 68°C   [████████░░░░░░░░░░░░]                                │
│ Cores  0: 31%  1: 46%  2: 77%  3: 22%  4: 55%  5: 39%  6: 81%  7: 18%        │
├──────────────────────────────────────────────────────────────────────────────┤
│ RAM   18.4 / 32.0 GB   57%   [███████████░░░░░░░░░]                          │
├──────────────────────────────────────────────────────────────────────────────┤
│ GPU   73%  Temp 71°C   [██████████████░░░░░░]                                │
│ VRAM   9.2 / 12.0 GB   76%   [███████████████░░░░░]                          │
├─────────────────────────── Top 10 Consuming ─────────────────────────────────┤
│ PID    USER       CPU%   RAM%  GPU%  VRAM     COMMAND                        │
│ 8123   giaime    188.0   12.1    65  7.8GB    python train.py                │
│ 2411   root       24.0    4.3     0    0MB    Xorg                           │
│ 9931   giaime     18.0    2.1     4  600MB    blender                        │
├──────────────────────────────────────────────────────────────────────────────┤
│ sort: cpu   paused: no   q quit   c/r/g/v/t sort   space pause   +/- rate    │
└──────────────────────────────────────────────────────────────────────────────┘
```

Column budget for the process table at 80 cols (inner width 78):

| Col      | Width | Notes                                  |
| -------- | ----- | -------------------------------------- |
| PID      | 6     | up to 999999                           |
| USER     | 10    | truncated with `…` if longer           |
| CPU%     | 6     | one decimal, may exceed 100            |
| RAM%     | 6     | one decimal                            |
| GPU%     | 5     | integer or `—`                         |
| VRAM     | 8     | e.g. `7.8GB` or `—`                    |
| COMMAND  | ~31   | remainder; truncated with `…` if longer|

**Shrinkage policy** (when terminal width < 80):

1. < 76 cols: drop VRAM column
2. < 70 cols: drop GPU% column
3. < 60 cols: drop USER column
4. < 50 cols: hide bar graphs; metrics remain as text
5. < 40 cols: refuse to render, show `terminal too narrow` message

---

## 4. CLI (final v0.1)

```text
USAGE:
    ztop [OPTIONS]

OPTIONS:
    -r, --refresh <SECONDS>   Refresh interval (0.25..5.0) [default: 1.0]
    -s, --sort <MODE>         Initial sort: cpu|ram|gpu|vram|total [default: cpu]
    -n, --top <N>             Number of processes to show [default: 10]
        --no-gpu              Skip GPU initialization entirely
        --no-temp             Skip temperature probes
    -h, --help                Print help
    -V, --version             Print version
```

---

## 5. Module layout

```text
src/
  main.rs            entrypoint, terminal setup/teardown
  cli.rs             clap definition, validation
  app.rs             AppState (sort, pause, refresh, frame counter), event loop
  metrics/
    mod.rs           Snapshot struct (everything one frame needs)
    cpu.rs           total + per-core, sysinfo
    memory.rs        RAM used/total, sysinfo
    temp.rs          hwmon + thermal_zone scan
    gpu.rs           NVML init, per-frame poll, per-process VRAM/util
    process.rs       merge sysinfo procs with NVML per-pid data; sort
  ui/
    mod.rs           top-level draw(); width-based layout decisions
    bars.rs          horizontal bar widget
    table.rs         process table widget
```

---

## 6. Refresh cadence and CPU cost budget

- The event loop polls input with a 100 ms timeout, then re-renders if either input arrived or a tick is due.
- Metric collection happens *only on tick*, not on every keypress, to keep idle CPU near zero.
- Self-budget target: < 1 % CPU on the host machine at 1 Hz with 8 logical cores.

---

## 7. Verification before tagging v0.1

- [ ] Runs as unprivileged user on a host *without* an NVIDIA GPU (GPU panel shows `n/a`, no panic).
- [ ] Runs as unprivileged user with NVIDIA GPU (GPU panel populated; per-process GPU% may be `—`).
- [ ] Runs as root with NVIDIA GPU (per-process GPU% populated).
- [ ] Survives terminal resize at every breakpoint in §3.
- [ ] `cargo clippy -- -D warnings` clean.
- [ ] Idle CPU < 1 % on an 8-core machine at default refresh.

---

© 2026 Andrea Bodei <info@andreabodei.com> — Dual-licensed MIT OR Apache-2.0.
