#!/usr/bin/env python3

import argparse
from pathlib import Path

COMPILED_PATHS = (
    b"/usr/lib/x86_64-linux-gnu/webkitgtk-6.0",
    b"/usr/lib/aarch64-linux-gnu/webkitgtk-6.0",
    b"/usr/lib/webkitgtk-6.0",
    b"/usr/lib64/webkitgtk-6.0",
    b"/usr/local/lib/x86_64-linux-gnu/webkitgtk-6.0",
    b"/usr/local/lib/webkitgtk-6.0",
)


def find_matches(data: bytes, path: bytes) -> list[tuple[int, int]]:
    matches = []
    start = 0
    while True:
        index = data.find(path, start)
        if index < 0:
            return matches
        end = index + len(path)
        if end == len(data) or data[end : end + 1] in (b"/", b"\x00"):
            matches.append((index, end))
        start = end


def padded_path(replacement: bytes, length: int) -> bytes:
    padding = length - len(replacement)
    if padding < 0:
        raise SystemExit("replacement is longer than the compiled path")
    if padding % 2:
        replacement += b"/"
        padding -= 1
    return replacement + b"/." * (padding // 2)


def relocate(path: Path, replacement: bytes) -> int:
    data = path.read_bytes()
    matches = []
    for compiled_path in COMPILED_PATHS:
        if len(compiled_path) < len(replacement):
            continue
        for start, end in find_matches(data, compiled_path):
            matches.append((start, end, compiled_path))

    if not matches:
        if replacement in data:
            return 0
        raise SystemExit(f"no WebKit helper path found in {path}")

    for start, end, original in reversed(matches):
        updated = padded_path(replacement, len(original))
        data = data[:start] + updated + data[end:]

    path.write_bytes(data)
    return len(matches)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("library", type=Path)
    parser.add_argument("--replacement", default="./w")
    args = parser.parse_args()
    replacement = args.replacement.encode()
    if not replacement.startswith(b"./"):
        raise SystemExit("replacement must be relative to the AppDir")
    count = relocate(args.library, replacement)
    print(f"relocated {count} WebKit helper path(s) in {args.library}")


if __name__ == "__main__":
    main()
