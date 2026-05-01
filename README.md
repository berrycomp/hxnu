# HXNU

![HXNU logo](docs/logo.jpg)

HXNU is the new Rust-based kernel line for Neonix. The old `heartix/kernel` tree remains the legacy reference implementation; new kernel bring-up starts here.

## Current Scope

- New repository dedicated to the kernel rewrite
- First release target: `2605` for May 2026
- Hybrid kernel direction
- First target: `x86_64`
- `aarch64` is planned for phase 2, after the x86_64 boot and memory path is stable
- POSIX and legacy Ghost support are planned as compatibility layers
- `init` startup and process handoff are expected to follow Linux or Unix-like conventions
- ABI compatibility with existing userspace is explicitly deferred until the native kernel core is stable

## Licensing

HXNU currently should be treated as `GPLv3-or-later`.

Reason: the current bootstrap code is not a clean-room implementation. It already reuses Ghost/Heartix-derived implementation material for boot protocol setup and early bring-up structure. See `NOTICE.md` for the current licensing position.

## Repository Layout

- `kernel/`: Rust kernel crate
- `boot/`: Limine configuration
- `scripts/`: bootstrap, ISO build, and QEMU run helpers
- `user/`: bootstrap init payloads and early userland experiments
- `docs/`: roadmap and architecture notes

Core design notes live in `docs/architecture.md`.

## Prerequisites

- `rustup`
- `xorriso`
- `qemu-system-x86_64`
- `curl`
- `tar`

## Bootstrap

```bash
./scripts/bootstrap.sh
./scripts/build-kernel.sh
```

`build-kernel.sh` prefers the external HXNU toolchain and builds the kernel as `x86_64-unknown-hxnu` when `hxnu-cargo` is available. It checks `HXNU_CARGO_BIN`, `hxnu-cargo` on `PATH`, and can bootstrap from a local compiler checkout at `../compilers/hxnu-rustc-compiler-x86_64`. If none are available, it falls back to Rust's built-in `x86_64-unknown-none` target with the same linker script.

For wrapper-only validation, set `HXNU_BUILD_DRIVER=hxnu` to make the script fail instead of falling back.

## Build A Bootable ISO

```bash
./scripts/build-iso.sh
```

`build-iso.sh` reuses `build-kernel.sh`, so ISO creation follows the same toolchain selection rules.

`prepare-limine.sh` first tries to reuse a local `../heartix/target/limine-9.2.0` tree during bootstrap. If that does not exist, it downloads the pinned Limine binary archive for `9.2.0` and stages the required artifacts under `vendor/`.

`build-initrd.sh` generates a small `cpio` `newc` archive and places it at `/boot/initrd.cpio` in the ISO. It now prefers the repo-local `hxnu-init` ELF payload under `user/init-zero/` for `/initrd/init`, falls back to the adjacent compiler workspace's `init-like` ELF example when needed, and finally falls back to the checked-in script placeholder. Set `HXNU_INITRD_INIT_MODE=elf|script|auto` to force the mode. Limine exposes the archive to the kernel as the `initrd` boot module.

## Run Under QEMU

```bash
./scripts/run-qemu.sh
```

`run-qemu.sh` prefers Homebrew's QEMU UEFI firmware when available and falls back to plain CD boot otherwise.

Expected first output on the serial console:

```text
HXNU: x86_64 early bootstrap
HXNU: Limine protocol handshake ok
```

When `/initrd/init` is staged as an ELF payload, later bring-up logs should also include:

```text
HXNU: exec syscall path=/initrd/init ...
HXNU: init exec-commit pid=1 comm=init ...
HXNU: init launch transfer path=/initrd/init ...
HXNU: init launch heartbeat tick=...
HXNU-INIT: payload online via int 0x80
HXNU: exec replace old-path=/initrd/init ...
HXNU: init exec-commit pid=1 comm=init cloexec-closed=1 ...
HXNU-INIT: abi=0x10000 pid=1 tid=1 comm=init stage=post-exec ...
HXNU-INIT: signal smoke sigaction=0 fork=10000 pending-before=yes handler=yes wait4=10000 status=0 pending-after=no
```

Those lines indicate the kernel entered the real HXNU `exec` syscall path for `/initrd/init`, committed an initial exec-style state reset for the bootstrap process, mapped the ELF load image plus a bootstrap stack with syscall headroom, transferred control into the loaded init image, closed a live `FD_CLOEXEC` descriptor during the second exec-commit, survived a payload-driven self-`exec` that replaced the previous lower-half image in place, and then exercised synthetic `fork/wait4` with observable `SIGCHLD` pending/clear behavior through `/proc/signals`.

## PT_INTERP Smoke Acceptance

```bash
./scripts/smoke-ptinterp.sh
```

This command builds a dedicated PT_INTERP smoke ISO, boots it under QEMU, and verifies that HXNU maps both the main ELF image and a separate interpreter ELF before transferring control to the interpreter entry point.

Expected acceptance lines include:

```text
HXNU: init load-prep path=/initrd/init ... interp=/initrd/interp-zero interp-src=/initrd/interp-zero interp-ok=yes ...
HXNU: init launch transfer path=/initrd/init launch=/initrd/interp-zero ...
HXNU-INTERP: abi=0x10000 pid=1 tid=1 argc=1 argv0=/initrd/init execfn=/initrd/init at-entry=0x0000000000503110 at-base=0x0000000000300000 at-phdr=0x0000000000500040 ...
```

Those lines show that the bootstrap `exec` stack now carries the main-program auxv view (`AT_ENTRY`, `AT_PHDR`, `AT_PHENT`, `AT_PHNUM`) while control is transferred to a distinct interpreter image with a non-zero `AT_BASE`.

## FAT Smoke Acceptance

```bash
./scripts/smoke-fat.sh
```

This command builds a synthetic GPT + FAT16 smoke image, boots it under QEMU, and verifies `/fat` mount visibility from kernel logs (`block`, `fat`, and `vfs` acceptance lines).

Expected FAT bring-up lines now also include:

```text
HXNU: fat preview file path=/fat/HELLO.TXT data=Hello HXNU!
HXNU: fat preview nested path=/fat/BIN/README.TXT data=Nested README
HXNU: fat preview lfn-root path=/fat/HELLO-LONG-NAME.TXT data=Long root file
HXNU: fat preview lfn-nested path=/fat/BIN/README-LONG-NAME.TXT data=Nested long README
```

Those lines show that the FAT mount is no longer limited to root-directory listing: HXNU can now traverse into subdirectories, read file content back through the normal VFS preview path, and assemble checksum-validated long filename chains from staged LFN directory entries.

Current bring-up logs also include:

- boot-relative timestamps
- HHDM and memory map summary
- UEFI framebuffer or GOP handoff summary
- output-only TTY console bootstrap with serial and framebuffer sinks
- multiple virtual TTY screen foundation with framebuffer redraw and active-console switching
- styled framebuffer console output for accent, success, warning, error, and fatal paths
- frame allocator and heap bootstrap summary
- GDT and IDT activation summary
- CPUID vendor, brand, leaf, and feature summary
- CPUID topology leaf summary from `0x0B/0x1F`
- local APIC capability and base-address probe
- local APIC timer one-shot self-test summary
- minimal ACPI discovery with `RSDP`, `XSDT`, `MADT`, and `FADT` summaries
- SMP topology inventory and AP bring-up target summary from `MADT`
- read-only `procfs` snapshot bootstrap
- read-only `devfs` namespace bootstrap
- dynamic block device nodes under `/dev` (`/dev/sdX`, `/dev/nvmeXn1`, `/dev/nvmXn` and partition variants) when block discovery is online
- minimal VFS mount and read facade for `/`, `/dev`, `/proc`, and `/initrd`
- normalized VFS path resolution and node lookup facade for mount-backed paths
- `cpio` `newc` initrd module discovery and `/initrd` read path
- `/initrd/init` executable candidate discovery with format probe
- `/initrd/init` load-prep inspection for shebang and ELF64 program headers
- ELF `PT_LOAD` vm-map planning summary with RWX permissions and zero-fill (BSS) accounting
- early Unix-like interpreter resolution fallback from `/bin/*` to `/initrd/bin/*`
- periodic scheduler tick bootstrap summary
- scheduler thread and runqueue model summary
- bootstrap to idle context-switch summary
- structured panic and fatal exception reports
- controlled exception self-test output

## Self-Tests

The x86_64 bring-up currently supports controlled boot-time self-tests.

- default: breakpoint
- page fault
- general protection fault
- kernel panic
- power reset

Examples:

```bash
./scripts/build-iso.sh
./scripts/run-qemu.sh
```

```bash
HXNU_CARGO_ARGS='--features exception-test-page-fault' ./scripts/build-iso.sh
./scripts/run-qemu.sh
```

```bash
HXNU_CARGO_ARGS='--features exception-test-general-protection' ./scripts/build-iso.sh
./scripts/run-qemu.sh
```

```bash
HXNU_CARGO_ARGS='--features panic-self-test' ./scripts/build-iso.sh
./scripts/run-qemu.sh
```

```bash
HXNU_CARGO_ARGS='--features power-reset-self-test' ./scripts/build-iso.sh
./scripts/run-qemu.sh
```

`power-reset-self-test` currently reaches the FADT reset-register write path on the default `q35` + OVMF + TCG test setup. On this host, serial output stops immediately after the `0xcf9` reset write, so treat it as partial validation until it is cross-checked on another platform.
