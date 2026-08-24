# TCOL / HPL (HXNU Public License)
# TCOL v1.1 / HPL (HXNU Public License)
# This file is strictly governed by the Tile Conservative Open License (TCOL v1.1).

cmake_minimum_required(VERSION 3.20)
project(HXNU_Kernel LANGUAGES C CXX)

# HXNU Architecture Configuration
set(CMAKE_C_STANDARD 11)
set(CMAKE_CXX_STANDARD 17)

# User Options (Chief Architect Vision)
option(USE_HXNU_COMPILER "Use zero-latency custom HXNU Compiler (Phase 7)" OFF)

if(USE_HXNU_COMPILER)
    message(STATUS "HXNU Custom Compiler Enabled. Bypassing Rustc.")
    set(RUST_COMPILER "/opt/hxnu/bin/hxnuc")
else()
    message(STATUS "Standard Rustc Enabled (Bare-Metal Fallback).")
    set(RUST_COMPILER "rustc")
endif()

# -----------------------------------------------------------------------------
# Toolchain pin.
#
# rust-toolchain.toml only applies inside the source tree. CMake invokes rustc
# from the build directory, where rustup falls back to the default toolchain --
# stable -- and every #![feature] then fails with E0554. Pin it explicitly for
# every rustc invocation below.
# -----------------------------------------------------------------------------
set(HXNU_RUST_TOOLCHAIN "nightly-2026-03-20" CACHE STRING "rustup toolchain used for all rustc calls")
set(RUSTC_ENV ${CMAKE_COMMAND} -E env "RUSTUP_TOOLCHAIN=${HXNU_RUST_TOOLCHAIN}")

# Kernel target: the HXNU custom target spec, not the stock triple.
set(KERNEL_TARGET_SPEC "${CMAKE_CURRENT_SOURCE_DIR}/x86_64-hxnu-none.json")
set(KERNEL_TARGET_NAME "x86_64-hxnu-none")
set(LINKER_SCRIPT "${CMAKE_CURRENT_SOURCE_DIR}/kernel/linker/x86_64.ld")

# Flags shared by every rustc call. codegen-units=1 is what makes the build
# byte-reproducible; strip=debuginfo is what keeps the image at parity with the
# old cargo output (552 KB rather than 1.4 MB). Do not change either casually.
set(HXNU_RUSTFLAGS
    -C opt-level=s
    -C codegen-units=1
    -C panic=abort
    -C strip=debuginfo
)

# -----------------------------------------------------------------------------
# Stage 1: sysroot.
#
# A custom target spec has no precompiled core, so core, compiler_builtins and
# alloc are built for it here. This is the work `cargo -Z build-std` used to do.
# Built once and reused; delete the build directory to force a rebuild.
# -----------------------------------------------------------------------------
set(HXNU_SYSROOT "${CMAKE_CURRENT_BINARY_DIR}/sysroot")
set(HXNU_SYSROOT_LIB "${HXNU_SYSROOT}/lib/rustlib/${KERNEL_TARGET_NAME}/lib")

execute_process(
    COMMAND ${CMAKE_COMMAND} -E env "RUSTUP_TOOLCHAIN=${HXNU_RUST_TOOLCHAIN}" rustc --print sysroot
    OUTPUT_VARIABLE RUST_SYSROOT OUTPUT_STRIP_TRAILING_WHITESPACE
)
set(RUST_LIB_SRC "${RUST_SYSROOT}/lib/rustlib/src/rust/library")

if(NOT EXISTS "${RUST_LIB_SRC}/core/src/lib.rs")
    message(FATAL_ERROR
        "rust-src component missing. Install it with:\n"
        "  rustup component add rust-src --toolchain ${HXNU_RUST_TOOLCHAIN}")
endif()

set(SYSROOT_CORE  "${HXNU_SYSROOT_LIB}/libcore.rlib")
set(SYSROOT_CB    "${HXNU_SYSROOT_LIB}/libcompiler_builtins.rlib")
set(SYSROOT_ALLOC "${HXNU_SYSROOT_LIB}/liballoc.rlib")

add_custom_command(
    OUTPUT ${SYSROOT_CORE}
    COMMAND ${CMAKE_COMMAND} -E make_directory ${HXNU_SYSROOT_LIB}
    COMMAND ${RUSTC_ENV} ${RUST_COMPILER} -Zunstable-options --edition 2024
        --crate-type rlib --crate-name core --target ${KERNEL_TARGET_SPEC}
        ${HXNU_RUSTFLAGS}
        ${RUST_LIB_SRC}/core/src/lib.rs -o ${SYSROOT_CORE}
    COMMENT "Sysroot 1/3: core (${KERNEL_TARGET_NAME})"
    VERBATIM
)

add_custom_command(
    OUTPUT ${SYSROOT_CB}
    COMMAND ${RUSTC_ENV} ${RUST_COMPILER} -Zunstable-options --edition 2024
        --crate-type rlib --crate-name compiler_builtins --target ${KERNEL_TARGET_SPEC}
        --cfg "feature=\"compiler-builtins\"" --cfg "feature=\"mem\""
        --extern core=${SYSROOT_CORE}
        ${HXNU_RUSTFLAGS}
        ${RUST_LIB_SRC}/compiler-builtins/compiler-builtins/src/lib.rs -o ${SYSROOT_CB}
    DEPENDS ${SYSROOT_CORE}
    COMMENT "Sysroot 2/3: compiler_builtins (mem intrinsics)"
    VERBATIM
)

add_custom_command(
    OUTPUT ${SYSROOT_ALLOC}
    COMMAND ${RUSTC_ENV} ${RUST_COMPILER} -Zunstable-options --edition 2024
        --crate-type rlib --crate-name alloc --target ${KERNEL_TARGET_SPEC}
        --extern core=${SYSROOT_CORE} --extern compiler_builtins=${SYSROOT_CB}
        ${HXNU_RUSTFLAGS}
        ${RUST_LIB_SRC}/alloc/src/lib.rs -o ${SYSROOT_ALLOC}
    DEPENDS ${SYSROOT_CORE} ${SYSROOT_CB}
    COMMENT "Sysroot 3/3: alloc"
    VERBATIM
)

add_custom_target(hxnu_sysroot DEPENDS ${SYSROOT_CORE} ${SYSROOT_CB} ${SYSROOT_ALLOC})

# -----------------------------------------------------------------------------
# Stage 2: AP trampoline.
#
# ap_boot.rs does include_bytes!(concat!(env!("OUT_DIR"), "/ap_trampoline.bin")),
# which cargo's build.rs used to satisfy. Without cargo, OUT_DIR must be set and
# the blob placed there by hand. Same yasm-or-prebuilt fallback build.rs had.
# -----------------------------------------------------------------------------
set(AP_OUT_DIR "${CMAKE_CURRENT_BINARY_DIR}/kernel-out")
set(AP_BLOB "${AP_OUT_DIR}/ap_trampoline.bin")
set(AP_ASM "${CMAKE_CURRENT_SOURCE_DIR}/kernel/src/arch/x86_64/ap_trampoline.asm")
set(AP_PREBUILT "${CMAKE_CURRENT_SOURCE_DIR}/kernel/src/arch/x86_64/ap_trampoline.bin")

find_program(YASM_BIN yasm)
if(YASM_BIN)
    add_custom_command(
        OUTPUT ${AP_BLOB}
        COMMAND ${CMAKE_COMMAND} -E make_directory ${AP_OUT_DIR}
        COMMAND ${YASM_BIN} -f bin -o ${AP_BLOB} ${AP_ASM}
        DEPENDS ${AP_ASM}
        COMMENT "Assembling: AP trampoline (yasm)"
        VERBATIM
    )
else()
    message(STATUS "yasm not found -- using the prebuilt AP trampoline blob")
    add_custom_command(
        OUTPUT ${AP_BLOB}
        COMMAND ${CMAKE_COMMAND} -E make_directory ${AP_OUT_DIR}
        COMMAND ${CMAKE_COMMAND} -E copy_if_different ${AP_PREBUILT} ${AP_BLOB}
        DEPENDS ${AP_PREBUILT}
        COMMENT "Staging: AP trampoline (prebuilt blob, yasm absent)"
        VERBATIM
    )
endif()

add_custom_target(ap_trampoline DEPENDS ${AP_BLOB})

# -----------------------------------------------------------------------------
# Stage 3: the kernel.
#
# rustc links directly against the linker script; there is no separate `ld`
# step. Producing an object and linking it afterwards with GNU ld -- what this
# file used to do -- bypassed the script that fixes the boot layout.
# -----------------------------------------------------------------------------
set(KERNEL_ELF "${CMAKE_CURRENT_BINARY_DIR}/hxnu-kernel")

file(GLOB_RECURSE KERNEL_SOURCES CONFIGURE_DEPENDS
     "${CMAKE_CURRENT_SOURCE_DIR}/kernel/src/*.rs")

add_custom_command(
    OUTPUT ${KERNEL_ELF}
    COMMAND ${CMAKE_COMMAND} -E env
        "RUSTUP_TOOLCHAIN=${HXNU_RUST_TOOLCHAIN}"
        "OUT_DIR=${AP_OUT_DIR}"
        ${RUST_COMPILER} -Zunstable-options --edition 2024
        --target ${KERNEL_TARGET_SPEC} --sysroot ${HXNU_SYSROOT}
        --crate-name hxnu_kernel --crate-type bin
        ${HXNU_RUSTFLAGS}
        -C force-frame-pointers=yes
        -C link-arg=-T${LINKER_SCRIPT}
        -C link-arg=--no-pie
        ${CMAKE_CURRENT_SOURCE_DIR}/kernel/src/main.rs -o ${KERNEL_ELF}
    DEPENDS ${KERNEL_SOURCES} ${LINKER_SCRIPT} ${KERNEL_TARGET_SPEC}
            ${SYSROOT_CORE} ${SYSROOT_CB} ${SYSROOT_ALLOC} ${AP_BLOB}
    COMMENT "Compiling: HXNU kernel (${KERNEL_TARGET_NAME}, cargo-free)"
    VERBATIM
)

add_custom_target(hxnu_kernel ALL DEPENDS ${KERNEL_ELF})
add_dependencies(hxnu_kernel hxnu_sysroot ap_trampoline)

# =============================================================================
# G-Boot AArch64 Bare-Metal Payload Target (RK3588S / aarch64-hxnu-none)
# =============================================================================

set(GBOOT_DIR "${CMAKE_CURRENT_SOURCE_DIR}/boot/gboot")
set(GBOOT_TARGET_SPEC "${GBOOT_DIR}/aarch64-hxnu-none.json")
set(GBOOT_LINKER_SCRIPT "${GBOOT_DIR}/scripts/linker_rk3588.ld")
set(GBOOT_CARGO_TOML "${GBOOT_DIR}/Cargo.toml")

if(CMAKE_BUILD_TYPE STREQUAL "Debug")
    set(CARGO_BUILD_FLAG "")
    set(GBOOT_CARGO_OUT_DIR "${GBOOT_DIR}/target/aarch64-hxnu-none/debug")
else()
    set(CARGO_BUILD_FLAG "--release")
    set(GBOOT_CARGO_OUT_DIR "${GBOOT_DIR}/target/aarch64-hxnu-none/release")
endif()

set(GBOOT_CARGO_ELF "${GBOOT_CARGO_OUT_DIR}/gboot")
set(GBOOT_FINAL_ELF "${CMAKE_CURRENT_BINARY_DIR}/gboot.elf")
set(GBOOT_FINAL_BIN "${CMAKE_CURRENT_BINARY_DIR}/gboot.bin")

find_program(OBJCOPY_BIN
    NAMES rust-objcopy aarch64-linux-gnu-objcopy llvm-objcopy objcopy
    HINTS "$ENV{HOME}/.cargo/bin" "/usr/bin" "/usr/local/bin"
)

if(NOT OBJCOPY_BIN)
    message(WARNING "Cross-arch objcopy not found! Defaulting to 'rust-objcopy'.")
    set(OBJCOPY_BIN "rust-objcopy")
endif()

message(STATUS "G-Boot payload extractor tool: ${OBJCOPY_BIN}")

add_custom_command(
    OUTPUT ${GBOOT_CARGO_ELF}
    COMMAND ${CMAKE_COMMAND} -E env "RUSTC_BOOTSTRAP=1"
        cargo build ${CARGO_BUILD_FLAG}
        --manifest-path ${GBOOT_CARGO_TOML}
        --target-dir ${GBOOT_DIR}/target
        -Z build-std=core,compiler_builtins
        --target ${GBOOT_TARGET_SPEC}
    WORKING_DIRECTORY ${GBOOT_DIR}
    DEPENDS
        ${GBOOT_CARGO_TOML}
        ${GBOOT_TARGET_SPEC}
        ${GBOOT_LINKER_SCRIPT}
        ${GBOOT_DIR}/src/main.rs
        ${GBOOT_DIR}/src/rk3588_regs.rs
        ${GBOOT_DIR}/src/pcie_ep.rs
        ${GBOOT_DIR}/src/handover.rs
    COMMENT "Compiling: G-Boot AArch64 Bare-Metal Payload (aarch64-hxnu-none)"
    VERBATIM
)

add_custom_command(
    OUTPUT ${GBOOT_FINAL_ELF} ${GBOOT_FINAL_BIN}
    COMMAND ${CMAKE_COMMAND} -E copy_if_different ${GBOOT_CARGO_ELF} ${GBOOT_FINAL_ELF}
    COMMAND ${OBJCOPY_BIN} -O binary ${GBOOT_FINAL_ELF} ${GBOOT_FINAL_BIN}
    DEPENDS ${GBOOT_CARGO_ELF}
    COMMENT "Extracting: Flashable G-Boot Binary Payload (gboot.bin) via ${OBJCOPY_BIN}"
    VERBATIM
)

add_custom_target(gboot ALL DEPENDS ${GBOOT_FINAL_ELF} ${GBOOT_FINAL_BIN})

# Linux Compatibility Layer (LCL) Module
add_custom_command(
    OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/liblcl.rlib
    COMMAND ${RUST_COMPILER}
        --edition 2021
        --target ${KERNEL_TARGET}
        --crate-type lib
        -C opt-level=3
        -C panic=abort
        -C relocation-model=static
        -C link-arg=-nostdlib
        ${CMAKE_CURRENT_SOURCE_DIR}/modules/lcl/src/lib.rs
        -o ${CMAKE_CURRENT_BINARY_DIR}/liblcl.rlib
    DEPENDS ${CMAKE_CURRENT_SOURCE_DIR}/modules/lcl/src/lib.rs
    COMMENT "Compiling: Linux Compatibility Layer (LCL)"
)

add_custom_target(lcl ALL DEPENDS ${CMAKE_CURRENT_BINARY_DIR}/liblcl.rlib)

# Rustybox MIT Edition Target
set(RUSTYBOX_DIR "${CMAKE_CURRENT_SOURCE_DIR}/vendor/rustybox")
set(RUSTYBOX_OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/rustybox_bin")

add_custom_command(
    OUTPUT ${RUSTYBOX_OUTPUT}
    COMMAND ${CMAKE_COMMAND} -E env "RUSTFLAGS=-C target-feature=+crt-static -C relocation-model=static -C panic=abort -C link-arg=-static"
        cargo build --manifest-path ${RUSTYBOX_DIR}/Cargo.toml
        --target x86_64-unknown-linux-musl
        --target-dir ${CMAKE_CURRENT_BINARY_DIR}/target-rustybox
        --release
    COMMAND ${CMAKE_COMMAND} -E copy
        ${CMAKE_CURRENT_BINARY_DIR}/target-rustybox/x86_64-unknown-linux-musl/release/rustybox
        ${RUSTYBOX_OUTPUT}
    COMMAND ${CMAKE_COMMAND} -E copy
        ${RUSTYBOX_OUTPUT}
        ${CMAKE_CURRENT_BINARY_DIR}/rustybox
    DEPENDS ${RUSTYBOX_DIR}/Cargo.toml ${RUSTYBOX_DIR}/src/main.rs
    COMMENT "Compiling: rustybox MIT Edition (Static x86_64-unknown-linux-musl)"
    VERBATIM
)

add_custom_target(rustybox ALL DEPENDS ${RUSTYBOX_OUTPUT})

# Initrd RootFS Target (Depends on rustybox)
add_custom_command(
    OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/initrd.cpio
    COMMAND ${CMAKE_CURRENT_SOURCE_DIR}/scripts/build-initrd.sh
    DEPENDS ${RUSTYBOX_OUTPUT} ${CMAKE_CURRENT_SOURCE_DIR}/scripts/build-initrd.sh
    COMMENT "Packaging: HXNU Initrd RootFS Archive (with rustybox)"
    VERBATIM
)

add_custom_target(initrd ALL DEPENDS ${CMAKE_CURRENT_BINARY_DIR}/initrd.cpio)

# HeterExec Target
set(HETEREXEC_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../heterexec")
set(HETEREXEC_OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/heterexec_bin")

add_custom_command(
    OUTPUT ${HETEREXEC_OUTPUT}
    COMMAND cargo build -Z build-std=core --release --target x86_64-unknown-none
    WORKING_DIRECTORY ${HETEREXEC_DIR}
    COMMAND ${CMAKE_COMMAND} -E copy_if_different
        ${HETEREXEC_DIR}/target/x86_64-unknown-none/release/heterexec
        ${HETEREXEC_OUTPUT}
    DEPENDS ${HETEREXEC_DIR}/Cargo.toml ${HETEREXEC_DIR}/src/lib.rs ${HETEREXEC_DIR}/src/loader.rs
    COMMENT "Compiling: HeterExec module (x86_64-unknown-none)"
    VERBATIM
)

add_custom_target(heterexec ALL DEPENDS ${HETEREXEC_OUTPUT})

# =============================================================================
# Sequential ordering.
#
# Required, not a preference. Three reasons, all specific to this tree:
#
#   1. heterexec/src/lib.rs is compiled both into the kernel (via #[path]) and
#      standalone as heterexec.hxext. Concurrent builds contend on the same
#      target directory and can read a source file mid-write.
#   2. The ABI between those two artifacts carries no version check, so a
#      deterministic order is what keeps them consistent.
#   3. Each stage embeds the previous one: initrd packs heterexec, the ISO packs
#      the initrd and the kernel. Built out of order, an image can ship a stale
#      module.
#
# Build with -j1. Do not raise codegen-units to compensate; that is what makes
# the output byte-reproducible.
# =============================================================================

add_dependencies(heterexec hxnu_kernel)
add_dependencies(initrd heterexec)
