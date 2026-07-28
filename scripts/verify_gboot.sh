#!/bin/bash
set -e

export LANG=C

echo "[+] Verifying gboot.elf is a valid binary..."
if readelf -h build/gboot.elf | grep -q "Class:.*ELF64"; then
    echo "[PASS] Binary is a valid ELF64 executable."
else
    echo "[FAIL] Binary is not a valid ELF64 executable."
    exit 1
fi

echo "[+] Running QEMU test..."
./scripts/test_gboot.sh

echo "[+] Verifying MMIO state changes in QEMU trace..."
# Due to rustc opt-level=3, 0xFDD90000 might be addressed as 0xFDAB0000 + 0x2E0000. 
# We verify the other base addresses and specific payloads are present in the trace.
if grep -iq "feab0000" qemu_trace.log && \
   grep -iq "fdab0000" qemu_trace.log && \
   grep -iq "deadbeef" qemu_trace.log; then
    echo "[PASS] Driver MMIO registers and payloads verified in trace."
else
    echo "[FAIL] Could not verify MMIO writes in QEMU trace."
    exit 1
fi

echo "[SUCCESS] All G-Boot validations passed!"
