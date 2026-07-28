# Neonix OS & HXNU Ecosystem Blueprint

This document outlines the finalized architectural roadmap for building the **Neonix Operating System** ecosystem from scratch. This is a hyper-ambitious, bare-metal lab project focusing on a micro-hybrid kernel (HXNU), heterogenous compute scaling (HPS/HFS), and zero-latency AI pipeline optimization bypassing all standard POSIX/Linux scheduler bottlenecks.

## Core Architectural Terminology
1. **Neonix:** The overarching Operating System ecosystem.
2. **HXNU:** The strictly independent Micro-Hybrid Kernel powering Neonix.

## Ecosystem Components

### 1. HXNU (Kernel)
- **Role:** Micro-hybrid kernel. Acts as the foundational bridge.
- **POSIX Philosophy:** HXNU retains the "good parts" of POSIX for high-level software portability and API familiarity. However, it strictly ABANDONS POSIX compliance when it comes to the internal **Scheduler** and **Memory Management**, utilizing custom bare-metal zero-latency algorithms instead.
- **Critical Goal:** **Fix and implement all missing Syscalls!** 
- **Internal Scheduler:** Contains its own foundational, zero-latency internal hardware scheduler to handle absolute bare-metal IRQs and context switching before handing off to HPS.
- **License:** MPL 2.0 + Apache 2.0

### 2. HPS (Heterogenous Processing Scheduler)
- **Role:** Sits as an independent layer **exactly between** HXNU (Kernel) and Neonix (Userspace), but it is **directly tied** to the HXNU internal hardware scheduler. It acts strictly as the software bridge for Neonix to handle high-level logic.
- **Function:** Acts as a unified scheduler that merges MPS + GMS + MLS into a single logical compute entity directly from the raw IRQ feeds of the HXNU internal scheduler.
- **Compute Routing:** Natively supports and routes **Vulkan** and **OpenCL** workloads across the heterogeneous hardware pool, ensuring zero-latency compute dispatch.
- **Bare-Metal Integration:** GMS, MLS, MPS, and the MaRTix core are NOT high-level libraries. They must be implemented from scratch as bare-metal routines directly inside HPS, and strictly tuned to the heterogeneous hardware scheduler (`sched.rs`) inside the HXNU kernel.
- **Security:** Contains a RISC-V direct translation layer and acts as a heterogeneous security layer. Monitored by a 1B Parameter LAM (Large Action Model) to protect raw syscalls and direct hardware (Framebuffer) communications.

### 3. HFS (Heterogenous File System)
- **Role:** A unified storage layer merging RAM + VRAM + SSD (strictly no HDD) into a single, cohesive, ultra-fast block device namespace.

### 4. Supernova (NVIDIA Driver)
- **Role:** A from-scratch, bare-metal graphics and compute driver for NVIDIA GPUs, explicitly targeting zero-latency GSP (GPU System Processor) synchronization.
- **MaRTix Core:** The MaRTix (Matrix Ray-Tracing) routing logic is embedded **directly** into the Supernova driver to guarantee absolute zero-latency dispatching to RT Cores.
- **Constraint:** Must only support Vulkan and the latest standards of OpenCL.

### 5. NeoIO
- **Role:** Hardware topology bridge converting ACPI tables into Flat Device Tree (FDT) to bypass poor virtualization/IOMMU support on standard x86 systems.

### 6. SECinter
- **Role:** Syscall-level Zero-Trust security layer utilizing entropy-based deterministic ID validation instead of standard randomized PIDs.

### 7. SXRC
- **Role:** Native HEX2/4 quantization/entropy compression module loaded directly into the OS memory map as an `.hxmd` file, allowing lossless tensor extraction.

### 8. paragon-core
- **Role:** The next-generation tensor and ML framework (replacing Candle) designed entirely as a `no_std` library to interface natively with HPS and the UMA allocator.

### 9. Linux Compatibility Layer (LCL)
- **Role:** A dedicated translation layer designed to guarantee native execution of standard Linux binaries (ELF) on Neonix.
- **Function:** Intercepts standard Linux syscalls and dynamically maps them to HXNU's high-speed, bespoke zero-latency APIs and HPS routing without requiring software recompilation.

### 10. Custom HXNU Compiler
- **Role:** A dedicated, custom-built compiler toolchain specifically designed for building the HXNU Kernel. 
- **Function:** Bypasses generic GCC/LLVM bottlenecks to aggressively optimize HXNU's bare-metal, zero-latency hardware routines and memory management models at compile time.

## Bootloader & Module Architecture
- **Initrd Minimalism:** The `/initrd` contains ONLY an in-kernel FAT32 driver. 
- **Module Layout:** All massive `.hxext` (HX Extension) and `.hxmd` (HX Module) drivers (like Supernova and SXRC) reside strictly in the `/boot` partition or the root (`/`) directory. They are mounted and loaded natively via the FAT32 driver during the boot sequence, completely bypassing standard `/lib/modules` bloat.
