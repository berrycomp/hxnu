#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ISO_ROOT="${ROOT}/build/iso"

"${ROOT}/scripts/build-dir.sh"

if [ -f "/usr/share/OVMF/OVMF_CODE_4M.fd" ]; then
    UEFI_CODE="/usr/share/OVMF/OVMF_CODE_4M.fd"
    UEFI_VARS_TEMPLATE="/usr/share/OVMF/OVMF_VARS_4M.fd"
elif [ -f "/usr/share/OVMF/x64/OVMF_CODE.4m.fd" ]; then
    UEFI_CODE="/usr/share/OVMF/x64/OVMF_CODE.4m.fd"
    UEFI_VARS_TEMPLATE="/usr/share/OVMF/x64/OVMF_VARS.4m.fd"
else
    UEFI_CODE="/usr/share/ovmf/OVMF.fd"
    UEFI_VARS_TEMPLATE="/usr/share/ovmf/OVMF.fd"
fi
UEFI_VARS="${ROOT}/build/OVMF_VARS.fd"

cp "${UEFI_VARS_TEMPLATE}" "${UEFI_VARS}"
qemu-system-x86_64 \
    -M q35,accel=tcg \
    -m 512M \
    -serial stdio \
    -display none \
    -drive if=pflash,format=raw,readonly=on,file="${UEFI_CODE}" \
    -drive if=pflash,format=raw,file="${UEFI_VARS}" \
    -drive file=fat:rw:"${ISO_ROOT}",format=raw,media=disk \
    -no-reboot \
    -no-shutdown > test.log 2>&1 &

QEMU_PID=$!
sleep 10
kill -9 "${QEMU_PID}" 2>/dev/null || true
cat test.log
