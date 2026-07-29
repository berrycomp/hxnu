#!/bin/bash
echo "Starting G-Boot test (QEMU)..."
rm -f serial.log qemu_trace.log
timeout 2 qemu-system-x86_64 -kernel build/gboot.elf -trace "memory_region_ops_write" -trace "memory_region_ops_read" -d guest_errors,invalid_mem -D qemu_trace.log -nographic -serial file:serial.log -no-reboot -m 64M -display none -vga none || true
