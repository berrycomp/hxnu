#!/bin/bash
echo "Starting G-Boot test (QEMU)..."
rm -f serial.log qemu_trace.log
timeout 2 qemu-system-x86_64 -kernel build/gboot.elf -d in_asm,invalid_mem -D qemu_trace.log -nographic -serial file:serial.log -no-reboot -m 64M -display none || true
