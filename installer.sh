#!/usr/bin/env bash
# Copyright (c) 2026 Andrea Bodei <info@andreabodei.com>
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# ZTOP installer — bootstraps Rust + build tools as needed, builds the release
# binary, and installs it as $PREFIX/bin/ztop.
#
# Usage:
#   ./installer.sh                   # bootstrap + build + install /usr/bin/ztop
#   PREFIX=/usr/local ./installer.sh # install /usr/local/bin/ztop instead
#   ./installer.sh --uninstall       # remove the installed binary
#   ./installer.sh --check           # print what's missing without changing anything
#   ./installer.sh --help            # show this help
#
# Run this installer as your normal user. It may ask for sudo for system-wide
# package installs and the final copy step, but it refuses to build as root so
# target/ never ends up root-owned. The installed ztop binary can still be run
# with sudo/root when NVML needs elevated privileges for per-process GPU usage.
# The NVIDIA driver is NOT auto-installed (kernel modules and secure boot make
# that unsafe to do from a shell script); we only detect it and warn if missing.

set -euo pipefail

PREFIX="${PREFIX:-/usr}"
BINDIR="$PREFIX/bin"
NAME="ztop"
TARGET="$BINDIR/$NAME"

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$HERE"

SUDO=""

# --- helpers --------------------------------------------------------------

log()  { printf '\033[1;36m==>\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m!! \033[0m %s\n' "$*" >&2; }
die()  { printf '\033[1;31mxx \033[0m %s\n' "$*" >&2; exit 1; }

refuse_root_build() {
    if [ "$(id -u)" -ne 0 ]; then
        return 0
    fi

    die "refusing to build as root because it would create root-owned target/ files; run ./installer.sh as your normal user, then use 'sudo ztop' when hardware requires root for GPU metrics"
}

need_sudo() {
    if [ -n "$SUDO" ] || [ "$(id -u)" -eq 0 ]; then
        return 0
    fi
    if command -v sudo >/dev/null 2>&1; then
        SUDO="sudo"
    else
        die "need root or sudo for system packages and writing to $BINDIR"
    fi
}

detect_pkg_manager() {
    if command -v apt-get >/dev/null 2>&1; then echo apt
    elif command -v dnf      >/dev/null 2>&1; then echo dnf
    elif command -v yum      >/dev/null 2>&1; then echo yum
    elif command -v pacman   >/dev/null 2>&1; then echo pacman
    elif command -v zypper   >/dev/null 2>&1; then echo zypper
    elif command -v apk      >/dev/null 2>&1; then echo apk
    else echo unknown
    fi
}

pkg_install() {
    # pkg_install <one or more package names for the detected PM>
    local pm
    pm=$(detect_pkg_manager)
    need_sudo
    case "$pm" in
        apt)    $SUDO apt-get update -y >/dev/null
                $SUDO apt-get install -y "$@" ;;
        dnf)    $SUDO dnf install -y "$@" ;;
        yum)    $SUDO yum install -y "$@" ;;
        pacman) $SUDO pacman -Sy --noconfirm "$@" ;;
        zypper) $SUDO zypper install -y "$@" ;;
        apk)    $SUDO apk add --no-cache "$@" ;;
        *)      die "no supported package manager found; install [$*] manually" ;;
    esac
}

# --- bootstrap stages -----------------------------------------------------

ensure_rust() {
    # If cargo isn't on the current shell's PATH but rustup put it under
    # ~/.cargo, source the env file first so we can detect a previous install
    # instead of redundantly reinstalling on every run.
    if ! command -v cargo >/dev/null 2>&1 && [ -f "$HOME/.cargo/env" ]; then
        # shellcheck source=/dev/null
        . "$HOME/.cargo/env"
    fi
    if command -v cargo >/dev/null 2>&1; then
        log "Rust toolchain present: $(rustc --version)"
        return 0
    fi
    log "Rust toolchain not found — installing via rustup (stable, minimal profile)"
    if ! command -v curl >/dev/null 2>&1; then
        log "curl not found; installing it first"
        pkg_install curl
    fi
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --default-toolchain stable --profile minimal
    if [ -f "$HOME/.cargo/env" ]; then
        # shellcheck source=/dev/null
        . "$HOME/.cargo/env"
    fi
    command -v cargo >/dev/null 2>&1 || \
        die "rustup ran but cargo is still not on PATH — open a new shell and re-run"
    log "Rust toolchain installed: $(rustc --version)"
    warn "to use 'cargo' in your shell after this script exits, run: . \"\$HOME/.cargo/env\" (or open a new shell)"
}

ensure_build_tools() {
    local missing=()
    command -v cc   >/dev/null 2>&1 || missing+=(cc)
    command -v make >/dev/null 2>&1 || missing+=(make)
    if [ ${#missing[@]} -eq 0 ]; then
        log "build tools present (cc, make)"
        return 0
    fi
    log "missing build tools: ${missing[*]} — installing"
    local pm
    pm=$(detect_pkg_manager)
    case "$pm" in
        apt)    pkg_install build-essential pkg-config ;;
        dnf)    pkg_install gcc make pkg-config ;;
        yum)    pkg_install gcc make pkgconfig ;;
        pacman) pkg_install base-devel ;;
        zypper) need_sudo; $SUDO zypper install -y -t pattern devel_basis ;;
        apk)    pkg_install build-base ;;
        *)      die "no supported package manager; install a C compiler and make manually" ;;
    esac
}

check_nvidia() {
    local nvidia_present=0
    if command -v nvidia-smi >/dev/null 2>&1; then
        local name
        name=$(nvidia-smi --query-gpu=name --format=csv,noheader 2>/dev/null | head -n1)
        log "NVIDIA detected: ${name:-unknown}"
        nvidia_present=1
    fi
    if [ $nvidia_present -eq 0 ]; then
        for d in /usr/lib/x86_64-linux-gnu /usr/lib64 /usr/lib /lib /lib64; do
            if [ -e "$d/libnvidia-ml.so.1" ]; then
                log "libnvidia-ml.so.1 found at $d (nvidia-smi missing but NVML is loadable)"
                nvidia_present=1
                break
            fi
        done
    fi
    if [ $nvidia_present -eq 0 ]; then
        warn "no NVIDIA driver detected (nvidia-smi missing, libnvidia-ml.so.1 not found)"
        warn "ztop will still run, but the GPU panel will show 'n/a'"
        warn "to enable GPU monitoring, install the NVIDIA driver via your distro repos"
        warn "(e.g. 'sudo apt install nvidia-driver-XXX' on Debian/Ubuntu, then reboot)"
        return 0
    fi

    # Ensure the unversioned NVML symlink exists. The runtime nvidia packages
    # only ship libnvidia-ml.so.1 (versioned soname); the unversioned
    # libnvidia-ml.so lives in the -dev package. nvml-wrapper's default open
    # uses the unversioned name, so without it ztop falls back to the .so.1
    # path — works, but installing the dev package is the conventional fix.
    local have_unversioned=0
    for d in /usr/lib/x86_64-linux-gnu /usr/lib64 /usr/lib /lib /lib64; do
        if [ -e "$d/libnvidia-ml.so" ]; then
            have_unversioned=1
            break
        fi
    done
    if [ $have_unversioned -eq 1 ]; then
        log "libnvidia-ml.so (unversioned) present"
        return 0
    fi
    local pm
    pm=$(detect_pkg_manager)
    case "$pm" in
        apt)
            # libnvidia-ml-dev exists on Ubuntu but not on stock Debian, where
            # the unversioned symlink ships only via the NVIDIA CUDA repo
            # (cuda-nvml-dev-XX-X). Check candidacy first so we don't surface
            # an "E: no installation candidate" error on Debian — the .so.1
            # fallback in ztop already handles that case.
            if apt-cache policy libnvidia-ml-dev 2>/dev/null | grep -q 'Candidate: [^(]'; then
                log "installing libnvidia-ml-dev (provides the unversioned libnvidia-ml.so symlink)"
                pkg_install libnvidia-ml-dev || warn "libnvidia-ml-dev install failed; ztop will use the .so.1 fallback"
            else
                log "libnvidia-ml-dev not available in this distro's repos; ztop will use the .so.1 fallback"
            fi
            ;;
        dnf|yum)
            warn "unversioned libnvidia-ml.so missing; ztop will use the .so.1 fallback"
            warn "(on Fedora/RHEL it's provided by cuda-nvml-devel-* from the CUDA repo)"
            ;;
        *)
            warn "unversioned libnvidia-ml.so missing; ztop will use the .so.1 fallback"
            ;;
    esac
}

# --- subcommands ----------------------------------------------------------

uninstall() {
    need_sudo
    if [ ! -e "$TARGET" ]; then
        warn "$TARGET does not exist; nothing to uninstall"
        exit 0
    fi
    log "removing $TARGET"
    $SUDO rm -f "$TARGET"
    log "done"
}

do_check() {
    log "environment audit (no changes made)"
    if command -v cargo >/dev/null 2>&1; then
        log "  cargo:        $(rustc --version)"
    else
        warn "  cargo:        missing (would install via rustup)"
    fi
    local missing=()
    command -v cc   >/dev/null 2>&1 || missing+=(cc)
    command -v make >/dev/null 2>&1 || missing+=(make)
    if [ ${#missing[@]} -eq 0 ]; then
        log "  build tools:  cc, make present"
    else
        warn "  build tools:  missing ${missing[*]} (would install via $(detect_pkg_manager))"
    fi
    if command -v nvidia-smi >/dev/null 2>&1; then
        log "  NVIDIA:       $(nvidia-smi --query-gpu=name --format=csv,noheader 2>/dev/null | head -n1)"
    else
        warn "  NVIDIA:       no driver detected (GPU panel would show 'n/a')"
    fi
    if [ -e "$TARGET" ]; then
        log "  target:       $TARGET exists ($($TARGET --version 2>/dev/null || echo unknown))"
    else
        log "  target:       $TARGET does not exist yet"
    fi
}

install_ztop() {
    refuse_root_build
    log "ZTOP installer — bootstrapping any missing dependencies"
    ensure_rust
    ensure_build_tools
    check_nvidia

    log "building release binary"
    cargo build --release --quiet

    BIN="$HERE/target/release/$NAME"
    [ -x "$BIN" ] || die "build did not produce $BIN"

    need_sudo
    if [ -e "$TARGET" ]; then
        log "$TARGET already exists; will replace"
    fi

    log "installing $TARGET (mode 0755)"
    $SUDO install -D -m 0755 "$BIN" "$TARGET"

    if "$TARGET" --version >/dev/null 2>&1; then
        log "installed: $($TARGET --version)"
    else
        warn "$TARGET installed but --version failed; check manually"
    fi
}

# --- entrypoint -----------------------------------------------------------

case "${1:-install}" in
    install)               install_ztop ;;
    --uninstall|uninstall) uninstall ;;
    --check|check)         do_check ;;
    -h|--help|help)        sed -n '2,21p' "$0" ;;
    *)                     die "unknown argument: $1 (try --help)" ;;
esac
