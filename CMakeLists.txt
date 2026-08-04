# TCOL / HPL (HXNU Public License)
# This file is strictly governed by the HXNU Public License (HPL).

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

# Kernel Target Definition
set(KERNEL_TARGET "x86_64-unknown-none")
set(LINKER_SCRIPT "${CMAKE_CURRENT_SOURCE_DIR}/scripts/linker.ld")

# Compiling the Core Kernel (Bypassing Cargo)
add_custom_command(
    OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/hxnu_kernel.o
    COMMAND ${RUST_COMPILER} 
        --edition 2021 
        --target ${KERNEL_TARGET} 
        --crate-type staticlib 
        -C opt-level=3 
        -C panic=abort
        -C relocation-model=static
        ${CMAKE_CURRENT_SOURCE_DIR}/kernel/src/main.rs 
        -o ${CMAKE_CURRENT_BINARY_DIR}/hxnu_kernel.o
        DEPENDS ${CMAKE_CURRENT_SOURCE_DIR}/kernel/src/main.rs
    COMMENT "Compiling: HXNU Kernel (Cargo bypassed, absolute memory linker active)."
)

add_custom_target(hxnu_kernel DEPENDS ${CMAKE_CURRENT_BINARY_DIR}/hxnu_kernel.o)

# Post-Build Hex / ELF Generation for G-Boot
# (This step links the object into the final immutable executable)
add_custom_command(
    TARGET hxnu_kernel POST_BUILD
    COMMAND ld -n -T ${LINKER_SCRIPT} -o ${CMAKE_CURRENT_BINARY_DIR}/hxnu.elf ${CMAKE_CURRENT_BINARY_DIR}/hxnu_kernel.o
    COMMENT "Linking: G-Boot Kexec Sequential Memory Layout Applied."
)

option(GBOOT_TARGET_KERNEL "Target Kernel (HXNU or LINUX)" "HXNU")

if(GBOOT_TARGET_KERNEL STREQUAL "LINUX")
    set(GBOOT_CFG "--cfg" "feature=\"load_linux\"")
else()
    set(GBOOT_CFG "--cfg" "feature=\"load_hxnu\"")
endif()

add_custom_command(
    OUTPUT ${CMAKE_CURRENT_BINARY_DIR}/gboot.o
    COMMAND ${RUST_COMPILER} 
        --edition 2021 
        --target ${KERNEL_TARGET} 
        --crate-type staticlib 
        -C opt-level=3 
        -C panic=abort
        -C relocation-model=static
        ${GBOOT_CFG}
        ${CMAKE_CURRENT_SOURCE_DIR}/boot/gboot/src/main.rs 
        -o ${CMAKE_CURRENT_BINARY_DIR}/gboot.o
        DEPENDS ${CMAKE_CURRENT_SOURCE_DIR}/boot/gboot/src/main.rs
    COMMENT "Compiling: G-Boot Bootloader (Target: ${GBOOT_TARGET_KERNEL})"
    VERBATIM
)

add_custom_target(gboot ALL DEPENDS ${CMAKE_CURRENT_BINARY_DIR}/gboot.o)

add_custom_command(
    TARGET gboot POST_BUILD
    COMMAND ld -n -T ${LINKER_SCRIPT} -o ${CMAKE_CURRENT_BINARY_DIR}/gboot.elf ${CMAKE_CURRENT_BINARY_DIR}/gboot.o
    COMMENT "Linking: G-Boot ELF Generated."
)

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



