# HXNU Roadmap & Neonix Blueprint

Release target:
- `2608` as the active first version marker for August 2026
- As of 2026-07-02, the architectural trajectory has been massively expanded to support the **Neonix Ecosystem** (HPS, HFS, Supernova, LCL, Custom Compiler).

## Phase 0 - Phase 2 (Completed Bootstraps)
- Separate Rust kernel repository, x86_64 target definition, Limine handoff.
- Frame allocator, Kernel heap, GDT/IDT, Exception handlers.
- APIC bring-up, base interrupt dispatch.
- **Current status:** CPUID, ACPI, SMP topology, basic VFS, FAT32 initrd discovery, and early `int 0x80` syscall dispatcher are online on `x86_64`.

## Phase 3: Kernel Virtualization & Syscall Foundation
- Virtual memory manager (VMM) with low latency.
- Page-fault resolution and trap-frame-aware user thread context save.
- Syscall entry path completion (Implementing all missing Ghost/POSIX syscalls).
- **Internal Hardware Scheduler:** Hardening the zero-latency, bare-metal internal IRQ scheduler.
- FAT32 support in-kernel for `/initrd` boot parsing.
- TTY core and UEFI framebuffer console plumbing.

## Phase 4: HPS Integration & Compatibility Layers
- **HPS (Heterogenous Processing Scheduler) Linkage:** Directly tying the internal hardware scheduler to the HPS software bridge.
- Stable SMP bring-up on `x86_64`.
- **Linux Compatibility Layer (LCL):** Intercepting Linux ELF syscalls and mapping them to HXNU/HPS APIs.
- Legacy Ghost compatibility layer.
- Driver discovery and load policy: Mounting `.hxext` and `.hxmd` drivers natively from `/boot` or `/`.

## Phase 5: The Heterogeneous Compute Era
- **Supernova NVIDIA Driver:** Loading the bare-metal GSP driver with embedded **MaRTix Core** for zero-latency Ray-Tracing.
- **ReDaemon DRIVER:** It will target the RX 6000 series and beyond, as the RT core acceleration will be implemented in a way similar to MaRTix in Supernova.
- **PhantomGP Driver:** This driver will be standard on all ARM GPUs from the Mali/Immortalis In Valhall architecture onwards. Ray tracing cores should also be supported, just like MaRTix in Supernova.
- **Rockchip SoC Support:** Support for all Rockchip SoCs from the generation starting with Rockchip RK3588 is required. Ray tracing cores should also be supported, just like MaRTix in Supernova & ReDeamon.
- **HFS (Heterogenous File System):** Merging RAM + VRAM + SSD into a unified namespace. Solutions need to be developed to address the power outage situation and prevent data loss.
- **SECinter:** Activating the Syscall-level Zero-Trust entropy-ID security layer.
- **Next-Gen Architecture Bring-up:** Initial direct translation and hardware bring-up for **ARMv9** (leveraging advanced SVE2 vector pipelines) and RISC-V.
- **HVL (Heterogeneous Virtualization Library):** A highly advanced virtualization layer that integrates with HPS and hardware schedulers, but also supports NeoIO and allows for the translation of many ISAs using LUT methods with MoltenEMU. Native or LUT translation is possible. **(NEONIX EXCLUSIVE)

## Phase 6: MVP Version of Neonix
- **Driver Support for Customers:** Support for essential I/O, Wi-Fi, audio, and Ethernet chips commonly found on consumer motherboards, such as Realtek, MediaTek, Intel, and Nuvoton. NOTE: Drivers will not be located in the same place; it is STRONGLY RECOMMENDED that they be written separately to minimize cumbersome processes.
- **Porting essential libraries & applications from Linux:** All essential packages such as gcc, build-essential, nxpkg (in the Neonix folder), flatpak, and Firefox must be ready, and cross-compilation libraries must be prepared.
- **Completion of the basic system components** such like nxinitd & nxsysd (systemd variant) with neonix-sdk.

## Phase 7: Global Domination
- **Custom HXNU Compiler:** Full migration to the bespoke, zero-latency optimizing HXNU compiler toolchain.
- **paragon-core:** Native `.hxmd` tensor execution library bypassing libc/rayon.
- **Future ISA Targets:** Full native compiler and scheduler support for advanced **ARMv9** topologies, **PowerISA (both Big-Endian and Little-Endian)**, and the bespoke **H16B (Dual-Pipeline 128-bit ISA: MPS + AVX-512 Hybrid)** architecture. Development and direct translation of the H16B hardware matrix pipelines will be driven exclusively by the Local Agent (Antigravity Operasyon Timi).
- Advanced Network & Audio stacks.
- Official rollout of the **HXNU Public License (HPL)** enforcement.

## Architecture Direction
- **Neonix** is the overarching ecosystem; **HXNU** is the micro-hybrid kernel.
- **Two-Tiered Scheduling:** HXNU handles raw bare-metal IRQs via its Internal Scheduler. This scheduler is directly tied to **HPS**, which acts as the intelligent software bridge for Neonix userspace (merging MPS, GMS, MLS).
- **POSIX Philosophy:** Retains POSIX API surfaces for LCL/software portability, but ABSOLUTELY ABANDONS POSIX compliance for internal scheduling and memory management.
- **Boot Minimalism:** `/initrd` contains ONLY an in-kernel FAT32 driver. Massive drivers load from `/boot` or root `/`.
- **Licensing:** The entire ecosystem is protected under the bespoke **HPL (HXNU Public License)**.

## Tier 1 Support List (HIGH PRIORITY)
- **H16B Custom 128 bit MultiVec (2x512 bit MVX) ISA**
- **All Explicitly parallel instruction computing (EPIC) based ISA's**
- **x86_64 / amd64 with AVX-512**
- **RISC-V 64 & 128 bit ISA**
- **ARMv9 Microrchitecture with aarch64 ISA & NEON SIMD**

# Tier 2 Support List (MEDUIM PRIORITY)
- **PowerISA LE/BE with AltiVec & CELL BE Support**
- **ARMv8 Microrchitecture with aarch64**
- **Elbrus 2000 ISA**
- **Other SIMD's for amd64**

# Tier 3 Support List (LOW PRIORITY)
- **MIPS**
- **SPARC**

# Tier 4 List (NON SUPPORTED)
- **ALL 32 bit CPU ISA's!!!**
- **Itanium (IA-64)**s

# FOLDER REQUIRED TO LOOK
- /home/eilhanzy/Projects/Neonix/
- /home/eilhanzy/Projects/neonix-uxperience/
- /home/eilhanzy/Projects/moltenemu/
- /home/eilhanzy/Projects/ParagonHPC/ && /home/eilhanzy/Projects/ParagonPE/
- /home/eilhanzy/Projects/Compilers/ **FOR HXNU**
