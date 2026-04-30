#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PACKAGE_NAME="hxnu-init"
TARGET_TRIPLE="x86_64-unknown-hxnu"
DEFAULT_COMPILER_REPO="${ROOT}/../compilers/hxnu-rustc-compiler-x86_64"
PRINT_PATH_ONLY=0

usage() {
    cat <<'EOF'
Usage: ./scripts/build-init-payload.sh [--print-path]
EOF
}

parse_args() {
    while [ "$#" -gt 0 ]; do
        case "$1" in
            --print-path)
                PRINT_PATH_ONLY=1
                ;;
            -h|--help)
                usage
                exit 0
                ;;
            *)
                echo "HXNU: unknown build-init-payload option: $1" >&2
                usage >&2
                exit 1
                ;;
        esac
        shift
    done
}

compiler_repo_root() {
    if [ -n "${HXNU_COMPILER_REPO:-}" ]; then
        printf '%s\n' "${HXNU_COMPILER_REPO}"
        return
    fi
    printf '%s\n' "${DEFAULT_COMPILER_REPO}"
}

hxnu_rustc_ready_for_binary() {
    local hxnu_cargo_bin="$1"
    local hxnu_cargo_dir
    hxnu_cargo_dir="$(cd "$(dirname "${hxnu_cargo_bin}")" && pwd)"

    if [ -x "${hxnu_cargo_dir}/hxnu-rustc" ]; then
        return 0
    fi

    command -v hxnu-rustc >/dev/null 2>&1
}

resolve_hxnu_cargo_runner() {
    if [ -n "${HXNU_CARGO_BIN:-}" ]; then
        if [ ! -x "${HXNU_CARGO_BIN}" ]; then
            echo "HXNU: HXNU_CARGO_BIN is not executable: ${HXNU_CARGO_BIN}" >&2
            exit 1
        fi
        if ! hxnu_rustc_ready_for_binary "${HXNU_CARGO_BIN}"; then
            echo "HXNU: HXNU_CARGO_BIN has no sibling hxnu-rustc and none is available on PATH" >&2
            exit 1
        fi
        printf 'binary:%s\n' "${HXNU_CARGO_BIN}"
        return 0
    fi

    if command -v hxnu-cargo >/dev/null 2>&1; then
        local path_hxnu_cargo
        path_hxnu_cargo="$(command -v hxnu-cargo)"
        if hxnu_rustc_ready_for_binary "${path_hxnu_cargo}"; then
            printf 'binary:%s\n' "${path_hxnu_cargo}"
            return 0
        fi
    fi

    local compiler_root
    compiler_root="$(compiler_repo_root)"
    if [ -f "${compiler_root}/Cargo.toml" ] && [ -f "${compiler_root}/crates/hxnu-cargo/Cargo.toml" ]; then
        printf 'cargo-run:%s\n' "${compiler_root}"
        return 0
    fi

    return 1
}

artifact_path() {
    printf '%s\n' "${ROOT}/target/${TARGET_TRIPLE}/release/${PACKAGE_NAME}"
}

build_with_hxnu_runner() {
    local runner="$1"
    local -a args=(
        build
        --manifest-path "${ROOT}/Cargo.toml"
        --release
        --target "${TARGET_TRIPLE}"
        -p "${PACKAGE_NAME}"
    )

    case "${runner}" in
        binary:*)
            local binary="${runner#binary:}"
            "${binary}" "${args[@]}"
            ;;
        cargo-run:*)
            local compiler_root="${runner#cargo-run:}"
            local hxnu_cargo_bin="${compiler_root}/target/debug/hxnu-cargo"
            (
                cd "${compiler_root}"
                cargo build -p hxnu-rustc -p hxnu-cargo
                "${hxnu_cargo_bin}" "${args[@]}"
            )
            ;;
        *)
            echo "HXNU: unsupported hxnu-cargo runner: ${runner}" >&2
            exit 1
            ;;
    esac
}

main() {
    parse_args "$@"

    if [ "${PRINT_PATH_ONLY}" -eq 1 ]; then
        artifact_path
        return
    fi

    local runner
    if ! runner="$(resolve_hxnu_cargo_runner)"; then
        echo "HXNU: failed to locate hxnu-cargo for init payload build" >&2
        echo "HXNU: checked HXNU_CARGO_BIN, PATH, and $(compiler_repo_root)" >&2
        exit 1
    fi

    echo "HXNU: building repo init payload with HXNU toolchain (${TARGET_TRIPLE})" >&2
    build_with_hxnu_runner "${runner}"

    if [ ! -f "$(artifact_path)" ]; then
        echo "HXNU: built init payload artifact is missing: $(artifact_path)" >&2
        exit 1
    fi
}

main "$@"
