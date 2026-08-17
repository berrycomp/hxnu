#!/usr/bin/env python3
# TCOL / HPL (HXNU Public License)
# This file is strictly governed by the HXNU Public License (HPL).

import os
import sys
from pathlib import Path

def make_fat_iso(src_path, out_path):
    src = Path(src_path)
    out = Path(out_path)
    if src.is_dir():
        data = bytearray(512 * 5064)
    elif src.exists():
        data = bytearray(src.read_bytes())
    else:
        data = bytearray(512 * 5064)

    SECTOR = 512
    part_start = 64
    part_sectors = 5000
    required_sectors = part_start + part_sectors
    required_bytes = required_sectors * SECTOR
    if len(data) < required_bytes:
        data.extend(b"\x00" * (required_bytes - len(data)))

    total_sectors = len(data) // SECTOR

    # GPT header at LBA1
    header = bytearray(SECTOR)
    header[0:8] = b"EFI PART"
    header[8:12] = (0x00010000).to_bytes(4, "little")
    header[12:16] = (92).to_bytes(4, "little")
    header[24:32] = (1).to_bytes(8, "little")
    header[32:40] = (total_sectors - 1).to_bytes(8, "little")
    header[40:48] = (34).to_bytes(8, "little")
    header[48:56] = (part_start + part_sectors - 1).to_bytes(8, "little")
    header[56:72] = bytes.fromhex("6a9c5d8fb4a34728a9d8d2b1a0c3f102")
    header[72:80] = (2).to_bytes(8, "little")
    header[80:84] = (128).to_bytes(4, "little")
    header[84:88] = (128).to_bytes(4, "little")
    data[SECTOR : 2 * SECTOR] = header

    # GPT entry array at LBA2
    entry_base = 2 * SECTOR
    entry = bytearray(128)
    entry[0:16] = bytes.fromhex("a2a0d0ebe5b9334487c068b6b72699c7")
    entry[16:32] = bytes.fromhex("de4f5bca7a0b4f2995e1666d0a4e2f11")
    entry[32:40] = (part_start).to_bytes(8, "little")
    entry[40:48] = (part_start + part_sectors - 1).to_bytes(8, "little")
    name = "HXNUFAT".encode("utf-16le")
    entry[56 : 56 + len(name)] = name
    data[entry_base : entry_base + 128] = entry

    # FAT16 BPB at partition start
    partition_base = part_start * SECTOR
    boot = bytearray(SECTOR)
    boot[0:3] = b"\xEB\x3C\x90"
    boot[3:11] = b"HXNUFAT "
    boot[11:13] = (512).to_bytes(2, "little")
    boot[13] = 1
    boot[14:16] = (1).to_bytes(2, "little")
    boot[16] = 2
    boot[17:19] = (512).to_bytes(2, "little")
    boot[19:21] = (part_sectors).to_bytes(2, "little")
    boot[21] = 0xF8
    boot[22:24] = (16).to_bytes(2, "little")
    boot[24:26] = (63).to_bytes(2, "little")
    boot[26:28] = (255).to_bytes(2, "little")
    boot[28:32] = (part_start).to_bytes(4, "little")
    boot[32:36] = (0).to_bytes(4, "little")
    boot[36] = 0x80
    boot[38] = 0x29
    boot[39:43] = (0x48584E55).to_bytes(4, "little")
    boot[43:54] = b"HXNUFAT    "
    boot[54:62] = b"FAT16   "
    boot[510] = 0x55
    boot[511] = 0xAA
    data[partition_base : partition_base + SECTOR] = boot

    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_bytes(data)

if __name__ == '__main__':
    src = None
    out = None
    if "-o" in sys.argv:
        idx = sys.argv.index("-o")
        if idx + 1 < len(sys.argv):
            out = sys.argv[idx + 1]
    for arg in sys.argv[1:]:
        if arg != out and not arg.startswith("-") and os.path.exists(arg):
            src = arg
    if not src:
        src = sys.argv[1] if len(sys.argv) > 1 and not sys.argv[1].startswith("-") else "/tmp"
    if not out:
        out = sys.argv[2] if len(sys.argv) > 2 and not sys.argv[2].startswith("-") else "/tmp/fat.iso"
    make_fat_iso(src, out)

