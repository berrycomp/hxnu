#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
KERNEL_PACKAGE="hxnu-kernel"
KERNEL_NAME="hxnu-kernel"
HXNU_TARGET_TRIPLE="x86_64-unknown-hxnu"
LEGACY_TARGET_TRIPLE="x86_64-unknown-none"
DEFAULT_COMPILER_REPO="${ROOT}/../compilers/hxnu-rustc-compiler-x86_64"
BUILD_DRIVER="${HXNU_BUILD_DRIVER:-auto}"
PRINT_PATH_ONLY=0
HXNU_EXTRA_ARGS=()
KERNEL_RUSTFLAGS=(
    "-C" "code-model=kernel"
    "-C" "relocation-model=static"
    "-C" "link-arg=-T${ROOT}/kernel/linker/x86_64.ld"
    "-C" "link-arg=--no-pie"
    "-C" "force-frame-pointers=yes"
)

usage() {
    cat <<'EOF'
Usage: ./scripts/build-kernel.sh [--print-path]

Build behavior:
- auto: prefer hxnu-cargo, fall back to cargo
- hxnu: require hxnu-cargo
- legacy: force plain cargo

Environment:
- HXNU_BUILD_DRIVER=auto|hxnu|legacy
- HXNU_CARGO_BIN=/path/to/hxnu-cargo
- HXNU_COMPILER_REPO=/path/to/hxnu-rustc-compiler-x86_64
- HXNU_CARGO_ARGS='--features panic-self-test'
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
                echo "HXNU: unknown build-kernel option: $1" >&2
                usage >&2
                exit 1
                ;;
        esac
        shift
    done
}

validate_build_driver() {
    case "${BUILD_DRIVER}" in
        auto|hxnu|legacy)
            ;;
        *)
            echo "HXNU: invalid HXNU_BUILD_DRIVER=${BUILD_DRIVER}; expected auto, hxnu, or legacy" >&2
            exit 1
            ;;
    esac
}

split_extra_args() {
    HXNU_EXTRA_ARGS=()
    if [ -n "${HXNU_CARGO_ARGS:-}" ]; then
        # shellcheck disable=SC2206
        HXNU_EXTRA_ARGS=(${HXNU_CARGO_ARGS})
    fi
}

compiler_repo_root() {
    if [ -n "${HXNU_COMPILER_REPO:-}" ]; then
        printf '%s\n' "${HXNU_COMPILER_REPO}"
        return
    fi
    printf '%s\n' "${DEFAULT_COMPILER_REPO}"
}

merged_kernel_rustflags() {
    local joined="${RUSTFLAGS:-}"
    local flag
    for flag in "${KERNEL_RUSTFLAGS[@]}"; do
        if [ -n "${joined}" ]; then
            joined="${joined} ${flag}"
        else
            joined="${flag}"
        fi
    done
    printf '%s\n' "${joined}"
}

hxnu_rustc_ready_for_binary() {
    local hxnu_cargo_bin="$1"
    local hxnu_cargo_dir
    hxnu_cargo_dir="$(cd "$(dirname "${hxnu_cargo_bin}")" && pwd)"

    if [ -x "${hxnu_cargo_dir}/hxnu-rustc" ]; then
        return 0
    fi

    if command -v hxnu-rustc >/dev/null 2>&1; then
        return 0
    fi

    return 1
}

resolve_hxnu_cargo_runner() {
    if [ "${BUILD_DRIVER}" = "legacy" ]; then
        return 1
    fi

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

selected_target_triple() {
    if resolve_hxnu_cargo_runner >/dev/null 2>&1; then
        printf '%s\n' "${HXNU_TARGET_TRIPLE}"
        return
    fi
    printf '%s\n' "${LEGACY_TARGET_TRIPLE}"
}

kernel_artifact_path() {
    local target_triple
    target_triple="$(selected_target_triple)"
    printf '%s\n' "${ROOT}/target/${target_triple}/release/${KERNEL_NAME}"
}

build_with_hxnu_runner() {
    local runner="$1"
    local -a args=(build --release --target "${HXNU_TARGET_TRIPLE}" -p "${KERNEL_PACKAGE}")
    local rustflags
    rustflags="$(merged_kernel_rustflags)"
    if [ "${#HXNU_EXTRA_ARGS[@]}" -gt 0 ]; then
        args+=("${HXNU_EXTRA_ARGS[@]}")
    fi

    case "${runner}" in
        binary:*)
            local binary="${runner#binary:}"
            RUSTFLAGS="${rustflags}" "${binary}" "${args[@]}"
            ;;
        cargo-run:*)
            local compiler_root="${runner#cargo-run:}"
            local hxnu_cargo_bin="${compiler_root}/target/debug/hxnu-cargo"
            (
                cd "${compiler_root}"
                cargo build -p hxnu-rustc -p hxnu-cargo
                RUSTFLAGS="${rustflags}" "${hxnu_cargo_bin}" "${args[@]}" --manifest-path "${ROOT}/Cargo.toml"
            )
            ;;
        *)
            echo "HXNU: unsupported hxnu-cargo runner: ${runner}" >&2
            exit 1
            ;;
    esac
}

build_with_legacy_cargo() {
    local -a args=(build --release --target "${LEGACY_TARGET_TRIPLE}" -p "${KERNEL_PACKAGE}")
    local rustflags
    rustflags="$(merged_kernel_rustflags)"
    if [ "${#HXNU_EXTRA_ARGS[@]}" -gt 0 ]; then
        args+=("${HXNU_EXTRA_ARGS[@]}")
    fi
    RUSTFLAGS="${rustflags}" cargo "${args[@]}"
}

main() {
    parse_args "$@"
    validate_build_driver
    split_extra_args

    if [ "${PRINT_PATH_ONLY}" -eq 1 ]; then
        kernel_artifact_path
        return
    fi

    local runner=""
    if runner="$(resolve_hxnu_cargo_runner)"; then
        echo "HXNU: building kernel with HXNU toolchain (${HXNU_TARGET_TRIPLE})" >&2
        build_with_hxnu_runner "${runner}"
        return
    fi

    if [ "${BUILD_DRIVER}" = "hxnu" ]; then
        echo "HXNU: failed to locate hxnu-cargo" >&2
        echo "HXNU: checked HXNU_CARGO_BIN, PATH, and $(compiler_repo_root)" >&2
        exit 1
    fi

    if [ "${BUILD_DRIVER}" = "legacy" ]; then
        echo "HXNU: building kernel with legacy cargo (${LEGACY_TARGET_TRIPLE})" >&2
    else
        echo "HXNU: hxnu-cargo unavailable; falling back to cargo (${LEGACY_TARGET_TRIPLE})" >&2
    fi
    build_with_legacy_cargo
}

main "$@"
