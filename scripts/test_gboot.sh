#!/bin/bash
echo "Starting G-Boot test (QEMU)..."
rm -f serial.log
timeout 2 qemu-system-x86_64 -kernel build/gboot.elf -nographic -serial file:serial.log -no-reboot -m 64M || true
