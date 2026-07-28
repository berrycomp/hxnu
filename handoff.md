# Empirical Challenger Report & Handoff

## 1. Observation
- Modified `scripts/test-heterexec.sh` to include `-d int` in QEMU arguments. Running it produced a QEMU log (`build/qemu-heterexec.log`) showing a kernel-mode Exception 14 (Page Fault) with `CR2=ffff8000fe000000` immediately after `HXNU: submitted workloads id1=1 id2=2`.
- Inspected `kernel/src/supernova.rs` and found the Iteration 2 fix added `hhdm_offset()` to `SUPERNOVA_MMIO_BASE` (`0xFE00_0000 + 0xffff800000000000`), but did not explicitly map this physical address in the page tables.
- Ran `./scripts/run-test.sh`. The `test.log` output shows `user-init` crashing repeatedly with: `user fault kind=page fault current=user-init#3 pid=3 rip=0xffffffff80001090 error=0x15 addr=0xffffffff80001090 action=kill-group`.
- Ran `objdump -d initrd/init` and verified the ELF is linked at a base address of `0xffffffff80000000` (`<_start>` is at `0xffffffff80000000`).
- Disassembly of `_start` shows indirect calls like `call *0x2ff7(%rip)`, which load absolute addresses (e.g., `0xffffffff80001090` for `memset`) stored in the static binary by the linker.

## 2. Logic Chain
1. The developer applied `hhdm_offset()` to `SUPERNOVA_MMIO_BASE`, generating the virtual address `0xffff8000fe000000`. However, Limine's Higher Half Direct Map (HHDM) exclusively maps physical RAM. `0xFE00_0000` is an MMIO region and remains unmapped. Dereferencing it during `ring_doorbell` results in a kernel-mode Page Fault.
2. The `user-init` executable is a static ELF linked with a kernel-space base address (`0xffffffff80000000`). The kernel's ELF loader (`kernel/src/init_exec.rs`) forces the load base to `USER_SPACE_BASE` (`0x400000`) but does not apply any dynamic relocations.
3. During execution at `0x400000`, `user-init` executes an indirect call (e.g., to `memset`) via an absolute pointer stored in the binary. This pointer points to `0xffffffff80001090`.
4. The CPU attempts an instruction fetch at `0xffffffff80001090` while in ring 3 (user mode). Since this address is in the kernel's higher half, the page table entry lacks the User-Accessible (`U/S`) flag, triggering a Page Fault (`error=0x15` meaning Present, User-mode, Instruction Fetch).

## 3. Caveats
- We assume Limine does not map MMIO in the HHDM, which aligns with standard Limine behavior (it only maps usable RAM and bootloader-reclaimable memory).
- The `user-init` executable might have been compiled by a custom toolchain (`x86_64-unknown-hxnu`), which currently defaults to a kernel-space load address (`0xffffffff80000000`) instead of a user-space PIE or `0x400000` base address.

## 4. Conclusion
**Verdict: FAIL**

The Iteration 2 fixes are inadequate. 
1. The `SUPERNOVA_MMIO_BASE` requires explicit page table mapping (e.g., using `crate::arch::x86_64::ensure_physical_region_mapped`) because the HHDM does not cover MMIO regions. Ringing the doorbell still crashes the kernel.
2. The `user-init` Page Fault persists because a higher-half linked static ELF is being loaded into lower-half user space without relocations. This causes absolute indirect jumps to branch into unexecutable supervisor memory.

## 5. Verification Method
To independently verify this bug:
1. Run `sed -i 's/-no-reboot \\/-no-reboot -d int \\/g' scripts/test-heterexec.sh && ./scripts/test-heterexec.sh` to observe the `CR2=ffff8000fe000000` Page Fault in `build/qemu-heterexec.log`.
2. Run `./scripts/run-test.sh` and `cat test.log | grep -ia "user fault"` to observe the user-mode instruction fetch page fault (`rip=0xffffffff80001090`, `error=0x15`).
3. Run `objdump -d initrd/init` to confirm it is linked at `0xffffffff80000000` and contains indirect calls to absolute higher-half addresses.
