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

echo "[+] Verifying execution success..."
if grep -q "Driver initialization complete" serial.log; then
    echo "[PASS] Driver initialization complete found."
else
    echo "[FAIL] Driver initialization complete not found."
    exit 1
fi

if grep -qi "chainloading" serial.log; then
    echo "[PASS] Chainloading successful."
else
    echo "[FAIL] Chainloading not found."
    exit 1
fi

echo "[+] Verifying hardware MMIO/Port writes via QEMU trace..."
for addr in b8000 3f8 60 3fd 64 3da; do
    if grep -iq "$addr" qemu_trace.log; then
        echo "[PASS] QEMU trace logged access for $addr."
    else
        echo "[FAIL] QEMU trace missing access for $addr."
        exit 1
    fi
done

echo "[SUCCESS] All G-Boot validations passed!"
