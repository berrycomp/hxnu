# HXNU Roadmap

Release target:
- `2605` as the first version marker for May 2026

## Phase 0
- Separate Rust kernel repository
- x86_64 target definition
- Minimal ELF kernel entry
- Early serial logging

## Phase 1
- Limine handoff wrappers
- Physical memory map parsing
- Early logging and panic reporting
- Frame allocator bootstrap
- Kernel heap bootstrap

## Phase 2
- GDT/IDT
- Exception handlers
- APIC or timer bring-up
- Interrupt dispatch
- Basic scheduler skeleton
- Structured kernel diagnostics and panic reports

Current status:
- GDT/IDT activation is online on `x86_64`
- CPUID inventory is online on `x86_64`
- CPUID topology leaf inventory from `0x0B/0x1F` is online on `x86_64`
- UEFI framebuffer or GOP handoff is online on `x86_64`
- Output-only TTY console bootstrap is online on `x86_64`
- Local APIC timer one-shot bring-up is online on `x86_64`
- Local APIC periodic tick and scheduler bootstrap are online on `x86_64`
- Minimal ACPI discovery with `RSDP`, `XSDT`, `MADT`, and `FADT` parsing is online on `x86_64`
- MADT processor, IO APIC, and interrupt-override topology summaries are online on `x86_64`
- FADT power and reset-register summaries are online on `x86_64`
- SMP topology inventory and AP bring-up target discovery are online on `x86_64`
- Read-only `procfs` snapshot bootstrap is online on `x86_64`
- Read-only `devfs` namespace bootstrap is online on `x86_64`
- Minimal VFS mount and read facade is online on `x86_64`
- VFS normalized path resolution and node lookup facade are online on `x86_64`
- `cpio` `newc` initrd discovery and `/initrd` read path are online on `x86_64`
- `/initrd/init` executable candidate discovery and format probe are online on `x86_64`
- `/initrd/init` ELF64 header and program-header inspection skeleton is online on `x86_64`
- `/initrd/init` ELF `PT_LOAD` vm-map planning with RWX and BSS accounting is online on `x86_64`
- `/initrd/init` ELF `PT_LOAD` segment materialization into zero-initialized vm-map buffers is online on `x86_64`
- `/initrd/init` handoff state now validates entry-on-executable-segment, maps staged ELF load images plus a bootstrap exec stack, commits an initial exec-style state reset (`comm`, `dumpable`, transient user mappings/TLS/rseq/robust-list state), and transfers control into the loaded init image on `x86_64`
- Bootstrap init handoff now enters through the real HXNU native `exec` syscall path (`process_exec_at`) for `/initrd/init` on `x86_64`
- Repo-local `hxnu-init` bootstrap payload is now staged into `initrd` during local builds and exercises HXNU-native `int 0x80` logging/identity/prctl syscalls after handoff on `x86_64`
- Bootstrap `hxnu-init` now performs a one-shot self-`exec`, proving lower-half image replacement and post-`exec` payload recovery on `x86_64`
- Bootstrap `hxnu-init` now leaves a `FD_CLOEXEC` tmpfs descriptor armed across self-`exec`, and the second exec-commit deterministically reports `cloexec-closed=1` on `x86_64`
- Bootstrap `hxnu-init` now exercises blocked self-signal queueing via `kill` + `rt_sigpending` and synthetic `fork/wait4` after self-`exec`, proving visible pending-signal mask behavior plus `SIGCHLD` notification before reap and clear-on-reap on `x86_64`
- Bootstrap `exec` now carries PT_INTERP launch metadata through stack construction (`launch_entry_point`, `AT_PHDR`, `AT_PHENT`, `AT_PHNUM`, `AT_BASE`), maps the main ELF image plus a distinct interpreter ELF when present, and is acceptance-covered by `scripts/smoke-ptinterp.sh` on `x86_64`
- Early Unix-like shebang interpreter fallback from `/bin/*` to `/initrd/bin/*` is online on `x86_64`
- Local ISO builds now stage a repo-local ELF init payload first, with compiler repo `init-like` fallback when needed, exercising the real `/initrd/init` ELF load + non-returning bootstrap handoff path on `x86_64`
- Partial Linux + Ghost + HXNU-native syscall compatibility dispatcher bootstrap is online on `x86_64`
- `x86_64` `int 0x80` syscall gate, register-frame dispatch, and entry self-test are online
- Bootstrap `uaccess` copyin/copyout validation facade is online on `x86_64`
- Bootstrap `openat/ioctl/access/newfstatat/faccessat/faccessat2/readlinkat/dup/dup2/dup3/fcntl/getcwd/chdir/fchdir/read/fstat/getdents64/lseek/close` (`Linux`) and `open/ioctl/access/stat/readlink/dup/dup2/dup3/fcntl/getcwd/chdir/fchdir/read/fstat/getdents/seek/close` (`Ghost`, `HXNU`) VFS-backed syscall paths are online
- `exit_group` syscall path is connected to scheduler thread-exit request handling
- Scheduler-backed `getpid/getppid/gettid` identity path is online for bootstrap syscall personalities
- Process-scoped `umask`, root-identity `getuid/getgid/geteuid/getegid`, and `set_tid_address` paths are online for Linux/Ghost/HXNU bootstrap personalities
- Bootstrap anonymous `mmap/mprotect/munmap` and process-scoped `brk` syscall facades are online for Linux/Ghost/HXNU personalities
- Bootstrap `nanosleep/gettimeofday/getrandom` syscall facades are online for Linux/Ghost/HXNU personalities
- Bootstrap `rt_sigaction/rt_sigprocmask` syscall facades are online for Linux/Ghost/HXNU personalities
- Bootstrap `pread64/pwrite64/readv/writev` syscall facades are online for Linux/Ghost/HXNU personalities
- Bootstrap `wait4/setpgid/getpgid/setsid/getsid` syscall facades are online for Linux/Ghost/HXNU personalities
- Bootstrap `getrlimit/setrlimit/prlimit64` syscall facades are online for Linux/Ghost/HXNU personalities
- Bootstrap `prctl(PR_SET_NAME/PR_GET_NAME/PR_SET_DUMPABLE/PR_GET_DUMPABLE)`, `set_robust_list/get_robust_list`, and `rseq` syscall facades are online for Linux/Ghost/HXNU personalities
- Bootstrap `arch_prctl(ARCH_SET_FS/ARCH_GET_FS/ARCH_SET_GS/ARCH_GET_GS)` and `futex(WAIT/WAKE)` syscall facades are online for Linux/Ghost/HXNU personalities
- Bootstrap `pipe/pipe2` and `poll/ppoll` syscall facades are online for Linux/Ghost/HXNU personalities
- Bootstrap `fork/vfork/clone` synthetic child-spawn and `wait4` child-reap facades are online for Linux/Ghost/HXNU personalities
- Bootstrap `exec` preflight (`execve/execveat` for Linux, `exec` for Ghost/HXNU) now validates path + `argv/envp` + loader compatibility before returning `ENOSYS` pending real image replacement
- `/proc/exec` endpoint now reports last exec preflight status (`ready/error`), path, mount, format, loader geometry, argument/environment sizing counters, and deferred `CLOEXEC` teardown forecast
- `/proc/signals` endpoint is online for bootstrap signal-mask/disposition/pending observability, and `kill` + `rt_sigpending` are online for masked pending-signal inspection alongside `SIGCHLD` tracking on synthetic child exit/reap
- ELF64 executable inspection now supports both little-endian and big-endian headers/program headers (`EI_DATA`) while keeping ELFCLASS64 enforcement
- Architecture-neutral vector capability facade is online with `x86_64` CPUID/XGETBV probing (`avx/avx2/avx512*`) and eager context policy selection (`xsave` or `fxsave`)
- `x86_64` scheduler context-switch path now preserves vector register state with eager `xsave/xrstor` or `fxsave/fxrstor` flow when available
- HXNU-native `cpucaps` syscall (`HXNU_SYS_CPUCAPS`) is online and exports stable base/ext feature bitmaps for user-space probing
- `/proc/vector` endpoint is online for vector capability/policy/context observability
- Accelerator driver contract bootstrap is online (`AccelDriverOps`), with CELL/B.E.-oriented SPE stub offload driver and `/proc/accel` telemetry
- Portability-level matrix for Linux/Unix-style userland bring-up is documented in `docs/portability-matrix.md`
- SXRC-derived memory compression integration contract is documented in `docs/sxrc-derivative-plan.md`
- `tools/sxrc-profile-gen` host-side skeleton and generated kernel profile artifact (`kernel/src/mm/compress/profile_generated.rs`) are online
- `mm/compress` backend trait contract and `NullBackend` bootstrap runtime facade are online on `x86_64`
- `mm/compress` bounded header/checksum codec path with `Zero/Same/Sxrc/Raw` classes is online, with deterministic `Raw` fallback on `x86_64`
- `mm/compress/store` fixed-capacity compressed-page slot store and accounting facade are online on `x86_64`
- `mm/pager` reclaim/restore path is online via compressed store with bootstrap roundtrip smoke on `x86_64`
- `/proc/compress` endpoint is online with compression runtime/store/pager observability counters on `x86_64`
- Bootstrap block-device layer is online with initrd-backed read-only sector facade on `x86_64`
- Block driver registry with `InitrdRamdisk` first driver contract is online on `x86_64`
- Dynamic block node aliases are online in `devfs` (`/dev/sdX`, `/dev/nvmeXn1`, `/dev/nvmXn` and partition variants)
- MBR signature probe and primary partition discovery scaffold are online via block layer on `x86_64`
- GPT header (`EFI PART`) probe and bounded entry discovery with MBR fallback are online on `x86_64`
- FAT16/32 read-only mount is online at `/fat`, with root-directory listing, subdirectory traversal, and file-content reads through the VFS path when a valid partition is present
- FAT long filename (`LFN`) assembly now validates staged sequence/checksum chains and is acceptance-covered for both root and nested files via `scripts/smoke-fat.sh`
- `/proc/block` now includes block driver and partition-table observability, and `/proc/fat` is online for FAT mount status
- Writable `tmpfs` bootstrap is online at `/tmp` and `/run` with file create/truncate/append paths plus constrained rename/unlink smoke coverage (`open/openat` + `read/write/pread64/pwrite64/writev` + `rename/unlink` on regular files)
- Tmpfs regular files now keep a stable file handle across rename/unlink while descriptors are open, with handle-scoped writeback/readback acceptance exercised by `hxnu-init` post-`exec` smoke
- Duplicated descriptors now share one open-file description for file offset and `F_SETFL` status while keeping `FD_CLOEXEC` descriptor-local, with `hxnu-init` acceptance covering `dup/dup3/fcntl`
- Open-file table ownership is now process-scoped, and `exit_group` purges owned descriptors
- `exit_group` now tears down the current thread-group and advances to the next runnable scheduler entry
- Ghost and HXNU-native parent-process identity calls are online (`getppid` / `process_parent`)
- Bootstrap per-CPU data-area registry is online on `x86_64`, with one frame-backed area per discovered CPU and `/proc/percpu` observability
- Multiple virtual TTY screen foundation is online on `x86_64`
- Scheduler thread table and runqueue skeleton are online on `x86_64`
- Bootstrap to idle-thread context switching is online on `x86_64`
- Styled framebuffer console output is online on `x86_64`
- Breakpoint, page fault, and general protection fault self-tests are working
- Power-reset self-test reaches the FADT reset-register path on `x86_64`
- Broader scheduler work remains next

Cross-repo status (as of 2026-03-29):
- External compiler repository `hxnu-rustc-compiler-x86_64` is online and versioned separately
- Rust-first SDK `v0.1.0` is tagged and includes `hxnu-rustc`, `hxnu-cargo`, `hxnu-sdk`, and `x86_64-unknown-hxnu` target spec
- SDK bundle flow (`build`, `pack`, `install`) and ELF verification flow are automated in the compiler repository
- Kernel integration model is consumer-style (`PATH` + `hxnu-cargo`), with no monorepo coupling
- Kernel build scripts now prefer `hxnu-cargo` and can also discover the adjacent compiler checkout during local development

## Phase 3
- Virtual memory manager
- Kernel virtual address-space management
- User virtual address-space management
- Page-fault resolution path
- Process and thread core
- Syscall entry path
- User-kernel memory copy and validation path
- IPC fast path
- ELF loader
- VFS core
- `devfs`
- `procfs`
- TTY core and console plumbing
- Multiple virtual TTY screens or virtual consoles
- Early keyboard or console input path
- UEFI framebuffer or GOP handoff and framebuffer console
- Block device layer
- Partition discovery
- cpio-compatible initrd support
- FAT16/32 support
- Minimal ACPI discovery on `x86_64`
- MADT and FADT parsing
- Reboot and poweroff plumbing
- Userspace ABI planning

## Phase 3.5
- Real `execve` path with user stack construction (`argv/envp/auxv`) and interpreter (`PT_INTERP`) handoff
- VFS core object model hardening (`inode`/`dentry`/open-file separation and descriptor lifecycle)
- Writable `tmpfs` bootstrap for early userland runtime paths (`/tmp`, `/run`)
- FAT v2 read-only expansion (file content reads, subdirectory traversal, staged LFN support)
- Syscall/process core hardening for real child runtime beyond synthetic spawn (`fork/clone` follow-up)
- Signal delivery baseline (`sigaction` wiring to scheduler/process state and `SIGCHLD` behavior)
- Acceptance focus: boot to userspace with `execve` + `PT_INTERP`, deterministic FD lifecycle, and stable `/dev` + `/proc` + `/fat` observability

## Phase 4
- SMP bring-up on `x86_64`
- BSP to AP startup flow
- Per-CPU data areas
- IPI support
- TLB shootdown path
- POSIX personality
- Legacy Ghost compatibility layer
- Core virtualization or LVE hooks
- Linux or Unix-like `init` startup contract
- PTY and POSIX terminal semantics
- Active TTY switching and console session routing
- Driver object model
- Device enumeration and bus framework
- Driver loading infrastructure for external driver directories
- Driver discovery and load policy for filesystem-backed modules
- Driver trust and load policy
- SXRC-derived compressed-page cache and reclaim backend
- ext4 driver
- exFAT driver

## Phase 5
- aarch64 bring-up
- PL011 early UART
- DTB parsing
- Exception vectors and GIC
- aarch64 SMP topology bring-up
- Heterogeneous CPU topology support
- big.LITTLE or hybrid-core scheduling awareness
- Basic Ethernet bring-up
- Early network driver model
- Loopback and packet path groundwork
- Minimal userspace networking boundary

## Phase 6
- Rust cross compiler support with `x86_64` and `aarch64` as first-class targets (`x86_64` bootstrap release is online in external compiler repo)
- C and C++ cross compiler support with `x86_64` and `aarch64` as first-class targets
- `musl` port for HXNU targets (`x86_64-unknown-hxnu`, later `aarch64-unknown-hxnu`) including `crt1/crti/crtn` and dynamic-linker contract
- `gcc-hxnu` cross toolchain support (`binutils` + GCC target integration) for freestanding and hosted profiles
- Additional architectures after the main two are stable
- PowerISA 64-bit bring-up
- Audio stack entry point
- Additional driver families loaded from external driver directories
- AHCI, NVMe, or virtio-blk expansion
- Richer Ethernet and audio driver families
- Debug monitor, symbol lookup, and crash dump direction

## Architecture Direction

- HXNU is a hybrid kernel
- Native HXNU primitives come first
- POSIX and legacy Ghost support are compatibility personalities, not the native kernel model
- Boot-critical and virtualization-critical pieces stay in kernel
- Replaceable services and policy should move to user space
- FAT16/32 can live in kernel if that keeps early boot and recovery simpler
- ext4 and exFAT are expected to work well as separate drivers or service modules
- `devfs` and `procfs` should arrive early with the VFS core
- TTY and framebuffer console support should be available before broader userspace compatibility work
- Multiple virtual TTY screens should sit between the early console path and full PTY/session semantics
- UEFI framebuffer support should be treated as a boot-critical display path
- Minimal ACPI discovery and power-state plumbing belong in kernel
- Full power-policy logic should stay outside the kernel when practical
- SMP comes before broad userspace compatibility work
- Heterogeneous CPU scheduling belongs after base SMP and timer stability
- The syscall and user-kernel boundary should be treated as a first-class kernel milestone
- Storage needs a block layer before filesystem work can scale
- Driver loading from dedicated filesystem directories should be supported after the base VFS and init path are stable

## Toolchain Priorities

- Rust cross compilation: `x86_64`, then `aarch64`
- C and C++ cross compilation: `x86_64`, then `aarch64`
- `musl` first-class userland libc for HXNU targets (headers, startup objects, and ABI-aligned sysroot packaging)
- `gcc-hxnu` and `binutils` support after Rust bootstrap, with compatibility checks against HXNU syscall/personality layers
- Other architectures only after the main two toolchains are reliable
- Compiler development continues in a dedicated repository: `https://github.com/neonix-bmx/hxnu-rustc-compiler-x86_64`
- Kernel repository tracks integration contract and acceptance checks, not compiler internals
