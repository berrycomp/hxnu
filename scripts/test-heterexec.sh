#!/usr/bin/env bash
set -euo pipefail

export PATH=$HOME/.local/bin:$PATH
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BUILD_DIR="${ROOT}/build"

if [ -f "${ROOT}/scripts/make_fat_iso.py" ]; then
    cp "${ROOT}/scripts/make_fat_iso.py" /tmp/make_fat_iso.py 2>/dev/null || true
    chmod +x /tmp/make_fat_iso.py 2>/dev/null || true
fi

export HXNU_CARGO_ARGS=""
"${ROOT}/scripts/build-iso.sh"

QEMU_PREFIX="$(brew --prefix qemu 2>/dev/null || true)"
if [ -n "${QEMU_PREFIX}" ]; then
    QEMU_SHARE_DIR="${QEMU_PREFIX}/share/qemu"
else
    QEMU_SHARE_DIR="/opt/homebrew/share/qemu"
fi
UEFI_CODE="${QEMU_SHARE_DIR}/edk2-x86_64-code.fd"
UEFI_VARS_TEMPLATE="${QEMU_SHARE_DIR}/edk2-i386-vars.fd"
UEFI_VARS="${BUILD_DIR}/edk2-x86_64-vars.fd"
ISO_PATH="${BUILD_DIR}/hxnu.iso"

LOG="${BUILD_DIR}/qemu-heterexec.log"
rm -f "${LOG}"

if [ -f "${UEFI_CODE}" ] && [ -f "${UEFI_VARS_TEMPLATE}" ]; then
    cp "${UEFI_VARS_TEMPLATE}" "${UEFI_VARS}"
    qemu-system-x86_64 \
        -M q35,accel=tcg \
        -m 512M \
        -serial stdio \
        -display none \
        -drive if=pflash,format=raw,readonly=on,file="${UEFI_CODE}" \
        -drive if=pflash,format=raw,file="${UEFI_VARS}" \
        -cdrom "${ISO_PATH}" \
        -no-reboot -d int \
        -no-shutdown > "${LOG}" 2>&1 &
else
    qemu-system-x86_64 \
        -M q35,accel=tcg \
        -m 512M \
        -serial stdio \
        -display none \
        -cdrom "${ISO_PATH}" \
        -no-reboot -d int \
        -no-shutdown > "${LOG}" 2>&1 &
fi

QEMU_PID=$!
sleep 15
kill -INT "${QEMU_PID}" 2>/dev/null || true
wait "${QEMU_PID}" 2>/dev/null || true

cat "${LOG}" | grep -a "HXNU:" || true
exit 0


