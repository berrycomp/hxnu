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

if grep -q "Driver initialization complete" serial.log; then
    echo "[PASS] G-Boot successfully executed driver initializations."
else
    echo "[FAIL] G-Boot did not execute properly or output was missing."
    cat serial.log
    exit 1
fi

echo "[SUCCESS] All G-Boot validations passed!"
