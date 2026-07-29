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

if grep -q "Chainloading HXNU" serial.log; then
    echo "[PASS] Chainloading successful."
else
    echo "[FAIL] Chainloading not found."
    exit 1
fi

echo "[+] Verifying no panic..."
if grep -qi "panic" serial.log; then
    echo "[FAIL] Panic found in log."
    exit 1
else
    echo "[PASS] No panic found."
fi

echo "[SUCCESS] All G-Boot validations passed!"
