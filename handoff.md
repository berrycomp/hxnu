# Empirical Challenger Report & Handoff

## 1. Observation
- Modified `kernel/src/main.rs` to include a `Heterexec` self-test (`SelfTest::Heterexec`) which initializes the `HSCHED`, submits two workloads (one CPU, one GPU), and calls `HSCHED.dispatch_pending()`.
- Built the HXNU kernel with `--features heterexec-self-test` and ran it under QEMU using a modified `scripts/test-heterexec.sh` script.
- The QEMU output (`qemu-heterexec.log`) abruptly stopped immediately after logging the submission:
  ```
  [0.144768975] HXNU: running kernel self-test = heterexec bridge
  [0.145560849] HXNU: heterexec hook shared_buffer ptr=0xffffffff800411e0
  [0.146700116] HXNU: submitted workloads id1=1 id2=2
  ```
- The expected continuation (`HXNU: dispatched workloads` and `HXNU: Heterexec bridge self-test PASSED`) was never printed, indicating a fatal fault (likely a Page Fault causing a triple fault because the exception handler might not have caught it properly, or QEMU stopped).
- Code inspection of `kernel/src/supernova.rs` shows that `SUPERNOVA_MMIO_BASE` is hardcoded as `0xFE00_0000 as *mut SupernovaHardware;`.
- `SupernovaDriver::ring_doorbell` writes directly to this address via `write_volatile(&mut (*self.mmio).doorbell, task_id);`.
- The boot logs show `HXNU: HHDM offset = 0xffff800000000000`, meaning physical memory is mapped in the higher half.

## 2. Logic Chain
1. The `Heterexec` self-test submits a `GpuCompute` task.
2. `HSCHED.dispatch_pending()` pulls this task and calls `self.supernova.ring_doorbell(task.id)`.
3. `ring_doorbell` attempts a raw memory write to the unmapped physical address `0xFE00_0000`.
4. Because the kernel operates with virtual memory enabled (using the Limine boot protocol and an HHDM offset), `0xFE00_0000` is an invalid virtual address in kernel space.
5. This invalid memory access triggers a Page Fault. Since the hardware at `0xFE00_0000` is not mapped via the HHDM offset (e.g., `0xffff800000000000 + 0xFE00_0000`) nor explicitly mapped in the page tables, the kernel crashes during the GPU task dispatch.

## 3. Caveats
- The test relies on QEMU's TCG execution. A real physical machine would still Page Fault because the physical address is not mapped into the virtual address space.
- CPU tasks are correctly queued but ignored in `dispatch_pending`, which is documented in the code (`// CPU tasks are handled elsewhere (ignored for now)`), so this behavior was not challenged.
- POSIX compatibility probes run by default during the bootstrap (`linux_probe`, `ghost_probe`) report successes in the logs (e.g. `linux_write=0`, `hxnu_abi_version=0x1`). The bug strictly lies in the MMIO dispatching bridge.

## 4. Conclusion
**Verdict: FAIL**

The `heterexec` hook and `HSCHED` bridge fail when dispatching GPU workloads because the `SupernovaDriver` attempts to write to an unmapped physical MMIO address (`0xFE00_0000`) instead of translating it into the kernel's virtual address space using the HHDM offset. This crashes the kernel.

**Blast radius**: HIGH. Any attempt by the `heterexec` layer to dispatch a GPU or SXRC task to the hardware via the `hps_bridge` will cause an immediate kernel panic or triple fault.

**Mitigation**: The `SupernovaDriver::new()` method must apply the HHDM offset to `SUPERNOVA_MMIO_BASE` (e.g., `SUPERNOVA_MMIO_BASE as u64 + hhdm_offset`) or the memory manager must explicitly map the `0xFE00_0000` MMIO region into the kernel's page tables before any doorbell rings are permitted.

## 5. Verification Method
To independently verify this bug:
1. Review the test harness in `/home/eilhanzy/Projects/hxnu/scripts/test-heterexec.sh` and the patched `kernel/src/main.rs`.
2. Run `./scripts/test-heterexec.sh`.
3. Check `/home/eilhanzy/Projects/hxnu/build/qemu-heterexec.log` and observe that execution halts abruptly during `dispatch_pending`.
4. Inspect `kernel/src/supernova.rs:10` to confirm the hardcoded unmapped physical address is used directly as a virtual pointer.
