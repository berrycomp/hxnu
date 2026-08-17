#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SOURCE_DIR="${ROOT}/initrd"
BUILD_DIR="${ROOT}/build"
STAGE_DIR="${BUILD_DIR}/initrd-root"
ARCHIVE_PATH="${BUILD_DIR}/initrd.cpio"

if [ -f "${ROOT}/scripts/make_fat_iso.py" ]; then
    cp "${ROOT}/scripts/make_fat_iso.py" /tmp/make_fat_iso.py 2>/dev/null || true
    chmod +x /tmp/make_fat_iso.py 2>/dev/null || true
fi

mkdir -p "${BUILD_DIR}"
rm -f "${ARCHIVE_PATH}"
rm -rf "${STAGE_DIR}"
mkdir -p "${STAGE_DIR}"

if [ -d "${SOURCE_DIR}" ]; then
    cp -R "${SOURCE_DIR}/." "${STAGE_DIR}/"
fi

HETEREXEC_DIR="${ROOT}/../heterexec"
if [ -d "${HETEREXEC_DIR}" ]; then
    (cd "${HETEREXEC_DIR}" && cargo build -Z build-std=core --release --target x86_64-unknown-none) || true
    mkdir -p "${STAGE_DIR}/boot"
    if [ -f "${HETEREXEC_DIR}/target/x86_64-unknown-none/release/heterexec" ]; then
        cp "${HETEREXEC_DIR}/target/x86_64-unknown-none/release/heterexec" "${STAGE_DIR}/boot/heterexec.hxext"
    fi
fi

if "${ROOT}/scripts/build-init-payload.sh"; then
    INIT_PAYLOAD="$("${ROOT}/scripts/build-init-payload.sh" --print-path)"
    if [ -f "${INIT_PAYLOAD}" ]; then
        cp "${INIT_PAYLOAD}" "${STAGE_DIR}/init"
        cp "${INIT_PAYLOAD}" "${STAGE_DIR}/init_old"
    fi
else
    echo "HXNU: init payload build unavailable, using initrd/init fallback" >&2
fi

LCL_DIR="${ROOT}/../Neonix/lcl"
if [ -d "${LCL_DIR}" ]; then
    if (cd "${LCL_DIR}" && rm -rf CMakeCache.txt CMakeFiles && cmake . && make lcl); then
        if [ -f "${LCL_DIR}/test_hello" ]; then
            cp "${LCL_DIR}/test_hello" "${STAGE_DIR}/test_hello"
        fi
    fi
fi

# Build & Stage rustybox MIT Edition
RUSTYBOX_BIN=""
if [ -f "${BUILD_DIR}/rustybox" ]; then
    RUSTYBOX_BIN="${BUILD_DIR}/rustybox"
elif [ -f "${BUILD_DIR}/target-rustybox/x86_64-unknown-linux-musl/release/rustybox" ]; then
    RUSTYBOX_BIN="${BUILD_DIR}/target-rustybox/x86_64-unknown-linux-musl/release/rustybox"
elif [ -f "${ROOT}/vendor/rustybox/target/x86_64-unknown-linux-musl/release/rustybox" ]; then
    RUSTYBOX_BIN="${ROOT}/vendor/rustybox/target/x86_64-unknown-linux-musl/release/rustybox"
else
    if [ -d "${ROOT}/vendor/rustybox" ]; then
        echo "HXNU: building rustybox for rootfs staging" >&2
        (cd "${ROOT}/vendor/rustybox" && RUSTFLAGS="-C target-feature=+crt-static -C relocation-model=static -C panic=abort -C link-arg=-static" cargo build --release --target x86_64-unknown-linux-musl) || true
        if [ -f "${ROOT}/vendor/rustybox/target/x86_64-unknown-linux-musl/release/rustybox" ]; then
            RUSTYBOX_BIN="${ROOT}/vendor/rustybox/target/x86_64-unknown-linux-musl/release/rustybox"
        fi
    fi
fi

if [ -n "${RUSTYBOX_BIN}" ] && [ -f "${RUSTYBOX_BIN}" ]; then
    echo "HXNU: staging rustybox from ${RUSTYBOX_BIN} into rootfs /bin" >&2
    mkdir -p "${STAGE_DIR}/bin"
    cp "${RUSTYBOX_BIN}" "${STAGE_DIR}/bin/rustybox"
    chmod +x "${STAGE_DIR}/bin/rustybox"
    for applet in sh ls cat echo pwd; do
        cp "${RUSTYBOX_BIN}" "${STAGE_DIR}/bin/${applet}"
        chmod +x "${STAGE_DIR}/bin/${applet}"
    done
fi

python3 "${ROOT}/scripts/make_cpio.py" "${STAGE_DIR}" "${ARCHIVE_PATH}"

