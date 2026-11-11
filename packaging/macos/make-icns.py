#!/usr/bin/env python3
"""Write a macOS .icns file from PNG files.

macOS's own iconutil only runs on macOS, so a bundle assembled elsewhere needs the icon built
here. The format is simple: the "icns" magic, the total length, then one typed chunk per size.

Usage: make-icns.py OUTPUT.icns SIZE=PNG [SIZE=PNG ...]
"""
import pathlib
import struct
import sys

TYPES = {16: b"icp4", 32: b"icp5", 64: b"icp6", 128: b"ic07", 256: b"ic08", 512: b"ic09", 1024: b"ic10"}


def build(sources, out):
    chunks = b""
    for size, path in sorted(sources.items()):
        if size not in TYPES:
            raise SystemExit(f"no icns type for {size}px")
        data = pathlib.Path(path).read_bytes()
        chunks += TYPES[size] + struct.pack(">I", len(data) + 8) + data
    pathlib.Path(out).write_bytes(b"icns" + struct.pack(">I", len(chunks) + 8) + chunks)
    return len(chunks)


def main(argv):
    if len(argv) < 3:
        raise SystemExit(__doc__)
    out = argv[1]
    sources = {}
    for arg in argv[2:]:
        size, _, path = arg.partition("=")
        sources[int(size)] = path
    n = build(sources, out)
    print(f"{out}: {n} bytes of chunks")


if __name__ == "__main__":
    main(sys.argv)
