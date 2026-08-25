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
    if (cd "${LCL_DIR}" && rm -rf CMakeCache.txt CMakeFiles && cmake . && make test_hello); then
        if [ -f "${LCL_DIR}/test_hello" ]; then
            cp "${LCL_DIR}/test_hello" "${STAGE_DIR}/test_hello"
        fi
    fi
fi

# Build eMachines E725 Native Drivers from Neonix
NEONIX_DRIVERS_DIR="${ROOT}/../Neonix/drivers"
if [ -d "${NEONIX_DRIVERS_DIR}" ]; then
    mkdir -p "${STAGE_DIR}/boot/drivers"
    
    # 1. GL40 Immortal Chipset
    if [ -d "${NEONIX_DRIVERS_DIR}/gl40_immortal_chipset" ]; then
        echo "HXNU: Building gl40_immortal_chipset..." >&2
        (cd "${NEONIX_DRIVERS_DIR}/gl40_immortal_chipset" && cargo +nightly-2026-03-20-x86_64-unknown-linux-gnu build -Z build-std=core --target x86_64-unknown-none --release) || true
        cp "${NEONIX_DRIVERS_DIR}/gl40_immortal_chipset/target/x86_64-unknown-none/release/libgl40_immortal_chipset.a" "${STAGE_DIR}/boot/drivers/gl40_immortal_chipset.hxext" 2>/dev/null || true
    fi

    # 2. ALC272 Audio
    if [ -d "${NEONIX_DRIVERS_DIR}/alc272" ]; then
        echo "HXNU: Building alc272 (eMachines Audio)..." >&2
        (cd "${NEONIX_DRIVERS_DIR}/alc272" && cargo +nightly-2026-03-20-x86_64-unknown-linux-gnu build -Z build-std=core --target x86_64-unknown-none --release) || true
        cp "${NEONIX_DRIVERS_DIR}/alc272/target/x86_64-unknown-none/release/libalc272.a" "${STAGE_DIR}/boot/drivers/alc272.hxext" 2>/dev/null || true
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

# Stage Heartix Userland Applications (.hxapp) and Kernel Extensions (.hxext)
HEARTIX_SYSROOT="${ROOT}/../heartix/build/sysroot"
if [ -d "${HEARTIX_SYSROOT}/applications" ]; then
    echo "HXNU: staging Heartix applications and drivers from ${HEARTIX_SYSROOT}/applications into rootfs" >&2
    mkdir -p "${STAGE_DIR}/applications"
    cp -R "${HEARTIX_SYSROOT}/applications/." "${STAGE_DIR}/applications/"
fi
if [ -d "${HEARTIX_SYSROOT}/system/lib" ]; then
    mkdir -p "${STAGE_DIR}/system/lib"
    cp -R "${HEARTIX_SYSROOT}/system/lib/." "${STAGE_DIR}/system/lib/"
fi

python3 "${ROOT}/scripts/make_cpio.py" "${STAGE_DIR}" "${ARCHIVE_PATH}"

