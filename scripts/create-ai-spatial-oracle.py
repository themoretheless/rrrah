#!/usr/bin/env python3
"""Render the pinned AI page with Poppler and record separate file/RGB hashes."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess

SOURCE_SHA256 = "c5fc34346629c3379cee6065c45b7de0b620be822604551d1463d8e691b03d06"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def create(source, output, executable):
    source = source.resolve(strict=True)
    if digest(source.read_bytes()) != SOURCE_SHA256:
        raise ValueError("source is not the pinned VectorApple fixture")
    version = subprocess.run([executable, "-v"], capture_output=True, text=True, check=True)
    # A fresh directory prevents replacement of an earlier oracle or evidence.
    output.mkdir(parents=False, exist_ok=False)
    command = [executable, "-r", "72", "-f", "1", "-l", "1", "-singlefile",
               str(source), str(output.resolve() / "poppler")]
    rendered = subprocess.run(command, capture_output=True, check=True)
    (output / "render.log").write_bytes(rendered.stdout + rendered.stderr)
    if digest(source.read_bytes()) != SOURCE_SHA256:
        raise ValueError("source changed during rendering")
    data = (output / "poppler.ppm").read_bytes()
    header = re.match(rb"P6\n(\d+) (\d+)\n255\n", data)
    if header is None or tuple(map(int, header.groups())) != (301, 246):
        raise ValueError("unexpected oracle dimensions or PPM header")
    rgb = data[header.end():]
    if len(rgb) != 301 * 246 * 3:
        raise ValueError("incomplete oracle RGB plane")
    metadata = {"source_sha256": SOURCE_SHA256,
                "poppler_version": (version.stdout + version.stderr).strip(),
                "dpi": 72, "page": 0, "dimensions": [301, 246],
                "rgb_sha256": digest(rgb), "ppm_sha256": digest(data),
                "command": command}
    with (output / "oracle.json").open("x") as destination:
        json.dump(metadata, destination, indent=2)
        destination.write("\n")
    return metadata


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--pdftoppm", default="pdftoppm")
    args = parser.parse_args()
    try:
        metadata = create(args.source, args.output_dir, args.pdftoppm)
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        parser.exit(2, f"oracle creation refused: {error}\n")
    print(json.dumps(metadata, indent=2))


if __name__ == "__main__":
    main()
