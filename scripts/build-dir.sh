#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VENDOR_VERSION="${LIMINE_VERSION:-9.2.0}"
LIMINE_DIR="${ROOT}/vendor/limine-${VENDOR_VERSION}"
ISO_ROOT="${ROOT}/build/iso"

"${ROOT}/scripts/prepare-limine.sh"
"${ROOT}/scripts/build-initrd.sh"
"${ROOT}/scripts/build-kernel.sh"

KERNEL_PATH="$("${ROOT}/scripts/build-kernel.sh" --print-path)"

rm -rf "${ISO_ROOT}"
mkdir -p "${ISO_ROOT}/boot/limine"
mkdir -p "${ISO_ROOT}/EFI/BOOT"
cp "${KERNEL_PATH}" "${ISO_ROOT}/boot/kernel"
cp "${ROOT}/build/initrd.cpio" "${ISO_ROOT}/boot/initrd.cpio"
cp "${ROOT}/boot/limine.conf" "${ISO_ROOT}/limine.conf"
cp "${ROOT}/boot/limine.conf" "${ISO_ROOT}/boot/limine/limine.conf"
cp "${LIMINE_DIR}/limine-bios.sys" "${ISO_ROOT}/limine-bios.sys"
cp "${LIMINE_DIR}/limine-bios.sys" "${ISO_ROOT}/boot/limine/limine-bios.sys"
if [ -f "${LIMINE_DIR}/BOOTX64.EFI" ]; then
    cp "${LIMINE_DIR}/BOOTX64.EFI" "${ISO_ROOT}/EFI/BOOT/BOOTX64.EFI"
fi
