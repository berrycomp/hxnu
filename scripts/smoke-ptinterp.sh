#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BUILD_DIR="${ROOT}/build"
BASE_ISO_ROOT="${BUILD_DIR}/iso"
VENDOR_VERSION="${LIMINE_VERSION:-9.2.0}"
LIMINE_DIR="${ROOT}/vendor/limine-${VENDOR_VERSION}"
COMPILER_ROOT="${HXNU_COMPILER_REPO:-${ROOT}/../compilers/hxnu-rustc-compiler-x86_64}"
TOOLCHAIN_BIN="${COMPILER_ROOT}/target/debug/hxnu-cargo"
TOOLCHAIN_PATH="${COMPILER_ROOT}/target/debug"
SMOKE_WORK_DIR="${BUILD_DIR}/ptinterp-smoke"
INTERP_TARGET_DIR="${SMOKE_WORK_DIR}/target-interp"
MAIN_TARGET_DIR="${SMOKE_WORK_DIR}/target-main"
SMOKE_INITRD="${SMOKE_WORK_DIR}/initrd-ptinterp.cpio"
SMOKE_ISO_ROOT="${SMOKE_WORK_DIR}/iso-root"
SMOKE_ISO="${BUILD_DIR}/hxnu-ptinterp-smoke.iso"
SMOKE_LOG="${BUILD_DIR}/qemu-ptinterp-smoke.log"
SMOKE_TIMEOUT="${HXNU_PTINTERP_SMOKE_TIMEOUT:-10}"
INTERP_ARTIFACT="${INTERP_TARGET_DIR}/x86_64-unknown-hxnu/release/hxnu-interp-zero"
MAIN_ARTIFACT="${MAIN_TARGET_DIR}/x86_64-unknown-hxnu/release/hxnu-init"

require_tool() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "HXNU: missing required tool: $1" >&2
        exit 1
    fi
}

assert_log() {
    local pattern="$1"
    if ! grep -En "$pattern" "${SMOKE_LOG}" >/dev/null 2>&1; then
        echo "HXNU: PT_INTERP smoke assertion failed: ${pattern}" >&2
        echo "HXNU: showing last log lines (${SMOKE_LOG})" >&2
        tail -n 120 "${SMOKE_LOG}" >&2 || true
        exit 1
    fi
}

assert_file() {
    local path="$1"
    if [ ! -f "${path}" ]; then
        echo "HXNU: expected artifact is missing: ${path}" >&2
        exit 1
    fi
}

ensure_toolchain() {
    if [ -x "${TOOLCHAIN_BIN}" ] && [ -x "${COMPILER_ROOT}/target/debug/hxnu-rustc" ]; then
        return
    fi

    (
        cd "${COMPILER_ROOT}"
        cargo build -p hxnu-rustc -p hxnu-cargo
    )
}

build_hxnu_package() {
    local package_name="$1"
    local target_dir="$2"
    local image_base="$3"
    local dynamic_linker="${4:-}"
    local rustflags="-C link-arg=--image-base=${image_base}"
    if [ -n "${dynamic_linker}" ]; then
        rustflags="${rustflags} -C link-arg=--dynamic-linker=${dynamic_linker}"
    fi

    (
        cd "${COMPILER_ROOT}"
        PATH="${TOOLCHAIN_PATH}:$PATH" \
        RUSTFLAGS="${rustflags}" \
        ./target/debug/hxnu-cargo build \
            --manifest-path "${ROOT}/Cargo.toml" \
            -p "${package_name}" \
            --release \
            --target x86_64-unknown-hxnu \
            --target-dir "${target_dir}"
    )
}

if ! [[ "${SMOKE_TIMEOUT}" =~ ^[0-9]+$ ]] || [ "${SMOKE_TIMEOUT}" -eq 0 ]; then
    echo "HXNU: HXNU_PTINTERP_SMOKE_TIMEOUT must be a positive integer (seconds)" >&2
    exit 1
fi

require_tool cpio
require_tool cargo
require_tool objdump
require_tool qemu-system-x86_64
require_tool xorriso

echo "HXNU: building baseline ISO"
"${ROOT}/scripts/build-iso.sh"

ensure_toolchain

rm -rf "${SMOKE_WORK_DIR}"
mkdir -p "${SMOKE_WORK_DIR}" "${SMOKE_ISO_ROOT}"
STAGE_DIR="$(mktemp -d "${SMOKE_WORK_DIR}/initrd-src.XXXXXX")"
trap 'rm -rf "${STAGE_DIR}"' EXIT

echo "HXNU: building PT_INTERP interpreter payload"
build_hxnu_package "hxnu-interp-zero" "${INTERP_TARGET_DIR}" "0x300000"
assert_file "${INTERP_ARTIFACT}"

echo "HXNU: building PT_INTERP main payload"
build_hxnu_package "hxnu-init" "${MAIN_TARGET_DIR}" "0x500000" "/initrd/interp-zero"
assert_file "${MAIN_ARTIFACT}"

if ! objdump -p "${MAIN_ARTIFACT}" | grep -Eq 'INTERP'; then
    echo "HXNU: PT_INTERP header is missing from ${MAIN_ARTIFACT}" >&2
    exit 1
fi
if ! strings -a "${MAIN_ARTIFACT}" | grep -Eq '^/initrd/interp-zero$'; then
    echo "HXNU: PT_INTERP path mismatch in ${MAIN_ARTIFACT}" >&2
    exit 1
fi

cp -R "${ROOT}/initrd/." "${STAGE_DIR}/"
cp "${MAIN_ARTIFACT}" "${STAGE_DIR}/init"
cp "${INTERP_ARTIFACT}" "${STAGE_DIR}/interp-zero"
chmod 0755 "${STAGE_DIR}/init" "${STAGE_DIR}/interp-zero"

(
    cd "${STAGE_DIR}"
    find . -print | LC_ALL=C sort | cpio -o -H newc --quiet > "${SMOKE_INITRD}"
)

cp -R "${BASE_ISO_ROOT}/." "${SMOKE_ISO_ROOT}/"
cp "${SMOKE_INITRD}" "${SMOKE_ISO_ROOT}/boot/initrd.cpio"

xorriso -as mkisofs -R -r -J -V HXNU \
    -b boot/limine/limine-bios-cd.bin \
    -no-emul-boot -boot-load-size 4 -boot-info-table -hfsplus \
    -apm-block-size 2048 \
    --efi-boot boot/limine/limine-uefi-cd.bin \
    -efi-boot-part --efi-boot-image \
    --protective-msdos-label \
    "${SMOKE_ISO_ROOT}" -o "${SMOKE_ISO}"

if [ -x "${LIMINE_DIR}/limine" ]; then
    "${LIMINE_DIR}/limine" bios-install "${SMOKE_ISO}"
fi

QEMU_PREFIX="$(brew --prefix qemu 2>/dev/null || true)"
if [ -n "${QEMU_PREFIX}" ]; then
    QEMU_SHARE_DIR="${QEMU_PREFIX}/share/qemu"
else
    QEMU_SHARE_DIR="/opt/homebrew/share/qemu"
fi
UEFI_CODE="${QEMU_SHARE_DIR}/edk2-x86_64-code.fd"
UEFI_VARS_TEMPLATE="${QEMU_SHARE_DIR}/edk2-i386-vars.fd"
UEFI_VARS="${BUILD_DIR}/edk2-x86_64-vars-ptinterp-smoke.fd"

rm -f "${SMOKE_LOG}"
if [ -f "${UEFI_CODE}" ] && [ -f "${UEFI_VARS_TEMPLATE}" ]; then
    cp "${UEFI_VARS_TEMPLATE}" "${UEFI_VARS}"
    qemu-system-x86_64 \
        -M q35,accel=tcg \
        -m 512M \
        -serial stdio \
        -display none \
        -drive if=pflash,format=raw,readonly=on,file="${UEFI_CODE}" \
        -drive if=pflash,format=raw,file="${UEFI_VARS}" \
        -cdrom "${SMOKE_ISO}" \
        -no-reboot \
        -no-shutdown > "${SMOKE_LOG}" 2>&1 &
else
    qemu-system-x86_64 \
        -M q35,accel=tcg \
        -m 512M \
        -serial stdio \
        -display none \
        -cdrom "${SMOKE_ISO}" \
        -no-reboot \
        -no-shutdown > "${SMOKE_LOG}" 2>&1 &
fi

QEMU_PID=$!
sleep "${SMOKE_TIMEOUT}"
kill -INT "${QEMU_PID}" 2>/dev/null || true
wait "${QEMU_PID}" 2>/dev/null || true

assert_log "HXNU: init load-prep .*interp=/initrd/interp-zero .*interp-src=/initrd/interp-zero .*interp-ok=yes"
assert_log "HXNU: exec syscall path=/initrd/init pid=1 argv=1 env=0 cloexec=0 format=elf"
assert_log "HXNU: init launch transfer path=/initrd/init launch=/initrd/interp-zero"
assert_log "HXNU-INTERP: abi=0x10000 .*argv0=/initrd/init .*execfn=/initrd/init .*at-entry=0x000000000050[0-9a-f]{4} .*at-base=0x0000000000300000 .*at-phdr=0x0000000000500040"

echo "HXNU: PT_INTERP smoke acceptance passed"
grep -En "HXNU: (init load-prep|exec syscall|init launch transfer)|HXNU-INTERP:" "${SMOKE_LOG}"
echo "HXNU: iso=${SMOKE_ISO}"
echo "HXNU: log=${SMOKE_LOG}"
