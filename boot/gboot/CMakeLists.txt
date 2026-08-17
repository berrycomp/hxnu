# TCOL / HPL (HXNU Public License)
# TCOL v1.1 / HPL (HXNU Public License)
# This file is strictly governed by the Tile Conservative Open License (TCOL v1.1).

cmake_minimum_required(VERSION 3.20)
project(gboot LANGUAGES C CXX)

set(GBOOT_DIR "${CMAKE_CURRENT_SOURCE_DIR}")
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
