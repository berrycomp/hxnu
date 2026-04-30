#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SOURCE_DIR="${ROOT}/initrd"
BUILD_DIR="${ROOT}/build"
STAGE_DIR="${BUILD_DIR}/initrd-stage"
ARCHIVE_PATH="${BUILD_DIR}/initrd.cpio"
LOCAL_INIT_BUILD="${ROOT}/scripts/build-init-payload.sh"
DEFAULT_COMPILER_REPO="${ROOT}/../compilers/hxnu-rustc-compiler-x86_64"
INIT_MODE="${HXNU_INITRD_INIT_MODE:-auto}"

build_repo_init_elf() {
    local artifact_path
    artifact_path="$("${LOCAL_INIT_BUILD}" --print-path)"

    "${LOCAL_INIT_BUILD}"
    cp "${artifact_path}" "${STAGE_DIR}/init"
    chmod 0755 "${STAGE_DIR}/init"
}

build_compiler_repo_init_like_elf() {
    local compiler_root="${HXNU_COMPILER_REPO:-${DEFAULT_COMPILER_REPO}}"
    local manifest_path="${compiler_root}/examples/init-like/Cargo.toml"
    local hxnu_cargo_bin="${compiler_root}/target/debug/hxnu-cargo"
    local artifact_path="${compiler_root}/target/x86_64-unknown-hxnu/release/init-like"

    if [ ! -f "${manifest_path}" ]; then
        return 1
    fi

    (
        cd "${compiler_root}"
        cargo build -p hxnu-rustc -p hxnu-cargo
        "${hxnu_cargo_bin}" build \
            --manifest-path "${manifest_path}" \
            --release \
            --target x86_64-unknown-hxnu
    )

    if [ ! -f "${artifact_path}" ]; then
        echo "HXNU: built init-like artifact is missing: ${artifact_path}" >&2
        exit 1
    fi

    cp "${artifact_path}" "${STAGE_DIR}/init"
    chmod 0755 "${STAGE_DIR}/init"
}

prepare_staging_tree() {
    rm -rf "${STAGE_DIR}"
    mkdir -p "${STAGE_DIR}"
    cp -R "${SOURCE_DIR}/." "${STAGE_DIR}/"
}

mkdir -p "${BUILD_DIR}"
rm -f "${ARCHIVE_PATH}"
prepare_staging_tree

case "${INIT_MODE}" in
    auto)
        if build_repo_init_elf; then
            echo "HXNU: initrd using repo ELF init payload" >&2
        elif build_compiler_repo_init_like_elf; then
            echo "HXNU: initrd using compiler repo ELF init-like payload" >&2
        else
            echo "HXNU: initrd using script placeholder init" >&2
        fi
        ;;
    elf)
        if build_repo_init_elf; then
            echo "HXNU: initrd using repo ELF init payload" >&2
        elif build_compiler_repo_init_like_elf; then
            echo "HXNU: initrd using compiler repo ELF init-like payload" >&2
        else
            echo "HXNU: failed to build ELF init payload from repo or ${HXNU_COMPILER_REPO:-${DEFAULT_COMPILER_REPO}}" >&2
            exit 1
        fi
        ;;
    script)
        echo "HXNU: initrd using script placeholder init" >&2
        ;;
    *)
        echo "HXNU: invalid HXNU_INITRD_INIT_MODE=${INIT_MODE}; expected auto, elf, or script" >&2
        exit 1
        ;;
esac

(
    cd "${STAGE_DIR}"
    find . -print | LC_ALL=C sort | cpio -o -H newc --quiet > "${ARCHIVE_PATH}"
)
