#!/bin/bash
echo "Starting G-Boot test (QEMU)..."
rm -f serial.log
timeout 2 qemu-system-x86_64 -kernel build/gboot.elf -d guest_errors -nographic -serial file:serial.log -no-reboot -m 64M -display none || true
