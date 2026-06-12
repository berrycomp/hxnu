#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SOURCE_DIR="${ROOT}/initrd"
BUILD_DIR="${ROOT}/build"
STAGE_DIR="${BUILD_DIR}/initrd-root"
ARCHIVE_PATH="${BUILD_DIR}/initrd.cpio"

mkdir -p "${BUILD_DIR}"
rm -f "${ARCHIVE_PATH}"
rm -rf "${STAGE_DIR}"
mkdir -p "${STAGE_DIR}"

cp -R "${SOURCE_DIR}/." "${STAGE_DIR}/"

if "${ROOT}/scripts/build-init-payload.sh"; then
    cp "$("${ROOT}/scripts/build-init-payload.sh" --print-path)" "${STAGE_DIR}/init"
else
    echo "HXNU: init payload build unavailable, using initrd/init fallback" >&2
fi

(
    cd "${STAGE_DIR}"
    find . -print | LC_ALL=C sort | cpio -o -H newc --quiet > "${ARCHIVE_PATH}"
)
