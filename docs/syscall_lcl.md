# Syscall LCL Foundation Architecture

## Overview
The Linux Compatibility Layer (LCL) in Neonix operates not as an in-kernel translation layer, but as an **independent userspace daemon**. It runs directly on top of the bare-metal `heterexec` heterogeneous compute scheduler. This ensures the HXNU Micro-Kernel remains absolutely zero-bloat while providing seamless Linux ELF compatibility.

## Interception Mechanism
- Hardware traps for Linux system calls (`int 0x80` and `syscall`) are intercepted natively by the HXNU kernel.
- The `syscall_handler` identifies these requests and delegates them to the `PosixLcl` ABI via the core dispatcher.
- The `SyscallAbi::PosixLcl` match arm routes the trap directly to the userspace LCL daemon, printing `[LCL Handler] Intercepted int 0x80\n` for debugging and verification purposes.
- The LCL daemon then safely translates these POSIX calls into the native HXNU/heterexec zero-latency primitives.

## Architectural Advantages
- **Security:** By pushing Linux syscall translation to userspace, we fortify the HXNU kernel against potential POSIX-specific attack vectors.
- **Latency:** Keeps the internal `hsched` (hardware scheduler) zero-latency by isolating backward compatibility from the critical interrupt path.
- **Modularity:** Seamlessly handles future ISA ports (ARMv9, RISC-V, H16B) because the heavy POSIX translation work is outsourced to the cross-compiled userspace daemon.
