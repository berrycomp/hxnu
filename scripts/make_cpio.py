#!/usr/bin/env python3
import os
import sys

def pack_cpio(stage_dir, output_file):
    with open(output_file, 'wb') as out:
        ino = 1
        items = []
        for root, dirs, files in os.walk(stage_dir):
            for name in dirs + files:
                full_path = os.path.join(root, name)
                rel_path = os.path.relpath(full_path, stage_dir).replace('\\', '/')
                if rel_path == '.':
                    continue
                rel_path = './' + rel_path
                items.append((rel_path, full_path))

        items.sort(key=lambda x: x[0])

        for rel_path, full_path in items:
            st = os.stat(full_path)
            is_dir = os.path.isdir(full_path)
            mode = st.st_mode
            filesize = 0 if is_dir else st.st_size
            namesize = len(rel_path.encode('utf-8')) + 1
            nlink = 2 if is_dir else 1
            mtime = int(st.st_mtime) & 0xffffffff

            header = (
                f"070701"
                f"{ino & 0xffffffff:08x}"
                f"{mode & 0xffffffff:08x}"
                f"00000000"  # uid
                f"00000000"  # gid
                f"{nlink:08x}"
                f"{mtime:08x}"
                f"{filesize & 0xffffffff:08x}"
                f"00000000"  # maj
                f"00000000"  # min
                f"00000000"  # rmaj
                f"00000000"  # rmin
                f"{namesize & 0xffffffff:08x}"
                f"00000000"  # chksum
            ).encode('ascii')

            assert len(header) == 110, f"Header size is {len(header)}, expected 110"
            out.write(header)
            name_bytes = rel_path.encode('utf-8') + b'\x00'
            out.write(name_bytes)
            pad_name = (4 - (len(header) + len(name_bytes)) % 4) % 4
            out.write(b'\x00' * pad_name)

            if not is_dir:
                with open(full_path, 'rb') as f:
                    data = f.read()
                    out.write(data)
                    pad_data = (4 - len(data) % 4) % 4
                    out.write(b'\x00' * pad_data)
            ino += 1

        trailer_name = b'TRAILER!!!\x00'
        header = (
            f"070701"
            f"{ino & 0xffffffff:08x}"
            f"00000000"  # mode
            f"00000000"  # uid
            f"00000000"  # gid
            f"00000001"  # nlink
            f"00000000"  # mtime
            f"00000000"  # filesize
            f"00000000"  # maj
            f"00000000"  # min
            f"00000000"  # rmaj
            f"00000000"  # rmin
            f"{len(trailer_name):08x}"
            f"00000000"  # chksum
        ).encode('ascii')
        assert len(header) == 110
        out.write(header)
        out.write(trailer_name)
        pad_name = (4 - (len(header) + len(trailer_name)) % 4) % 4
        out.write(b'\x00' * pad_name)

if __name__ == '__main__':
    pack_cpio(sys.argv[1], sys.argv[2])
