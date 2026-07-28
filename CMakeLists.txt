cmake_minimum_required(VERSION 3.20)
project(HXNU_Kernel LANGUAGES C CXX)

# HXNU Architecture Configuration
set(CMAKE_C_STANDARD 11)
set(CMAKE_CXX_STANDARD 17)

# User Options (Baş Mimar Vizyonu)
option(USE_HXNU_COMPILER "Faz 7'de geliştirilecek sıfır-gecikmeli özel HXNU Derleyicisini Kullan" OFF)

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
        --emit obj 
        -C opt-level=3 
        -C panic=abort
        -C relocation-model=static
        -C link-arg=-T${LINKER_SCRIPT}
        -C link-arg=-nostdlib
        ${CMAKE_CURRENT_SOURCE_DIR}/kernel/src/lib.rs 
        -o ${CMAKE_CURRENT_BINARY_DIR}/hxnu_kernel.o
    DEPENDS ${CMAKE_CURRENT_SOURCE_DIR}/kernel/src/lib.rs
    COMMENT "Derleme: HXNU Çekirdeği (Cargo bypass edildi, absolute memory linker devrede)."
)

add_custom_target(hxnu_kernel ALL DEPENDS ${CMAKE_CURRENT_BINARY_DIR}/hxnu_kernel.o)

# Post-Build Hex / ELF Generation for G-Boot
# (This step links the object into the final immutable executable)
add_custom_command(
    TARGET hxnu_kernel POST_BUILD
    COMMAND ld -n -T ${LINKER_SCRIPT} -o ${CMAKE_CURRENT_BINARY_DIR}/hxnu.elf ${CMAKE_CURRENT_BINARY_DIR}/hxnu_kernel.o
    COMMENT "Linking: G-Boot Kexec Sequential Memory Layout Applied."
)
