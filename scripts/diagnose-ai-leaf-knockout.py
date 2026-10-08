#!/usr/bin/env python3
"""External leaf knockout experiment using native child color and shape renders.

Requires pypdf and numpy. Specific diagnostic for isolated group-05 of the
pinned VectorApple corpus; never qualifies the full source or edits production.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import numpy as np
from pypdf import PdfReader, PdfWriter
from pypdf.generic import ContentStream, NameObject, NumberObject


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("--dump", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    args = parser.parse_args()
    args.output_dir.mkdir(exist_ok=False)
    reader = PdfReader(args.source)
    form = reader.pages[0]["/Resources"]["/XObject"]["/Fm1"].get_object()
    operations = ContentStream(form, reader).operations
    groups = []
    depth = 0
    for index, (_, op) in enumerate(operations):
        if op == b"q":
            if depth == 0:
                start = index
            depth += 1
        elif op == b"Q":
            depth -= 1
            if depth == 0:
                groups.append((start, index + 1))
    if depth or len(groups) != 17:
        raise ValueError("unexpected leaf structure")
    result = None
    inputs = []
    for index, (start, end) in enumerate(groups):
        rendered = {}
        for mode in ("color", "shape"):
            writer = PdfWriter()
            writer.clone_document_from_reader(PdfReader(args.source))
            target = writer.pages[0]["/Resources"]["/XObject"]["/Fm1"].get_object()
            content = ContentStream(None, writer)
            content.operations = operations[start:end]
            target._data = content.get_data()
            for key in ("/Filter", "/DecodeParms"):
                if key in target:
                    del target[key]
            if mode == "shape":
                for operands, op in operations[start:end]:
                    if op == b"gs":
                        state = target["/Resources"]["/ExtGState"][operands[0]].get_object()
                        state[NameObject("/ca")] = NumberObject(1)
                        state[NameObject("/CA")] = NumberObject(1)
            prefix = args.output_dir / f"child-{index:02}-{mode}"
            with prefix.with_suffix(".pdf").open("xb") as output:
                writer.write(output)
            metadata = json.loads(subprocess.check_output([str(args.dump.resolve()),
                str(prefix.with_suffix(".pdf")), "0", str(prefix.with_suffix(".rgba"))], text=True))
            rgba = np.frombuffer(prefix.with_suffix(".rgba").read_bytes(), dtype=np.uint8).reshape(metadata["height"], metadata["width"], 4).astype(np.uint32)
            rgba[:, :, :3] = (rgba[:, :, :3] * rgba[:, :, 3:] + 127) // 255
            rendered[mode] = rgba
            inputs.append({"child": index, "mode": mode, "pdf_sha256": hashlib.sha256(prefix.with_suffix(".pdf").read_bytes()).hexdigest()})
        color = rendered["color"]
        shape = rendered["shape"][:, :, 3:]
        if np.any(color[:, :, 3:] > shape):
            raise ValueError("child alpha exceeds shape coverage")
        if result is None:
            result = np.zeros_like(color)
        # Initial transparent group contribution; white page composite follows.
        result = (255 * color + (255 - shape) * result + 127) // 255
        print(f"composited child {index}", flush=True)
    actual = (result[:, :, :3] + 255 - result[:, :, 3:]).astype(np.uint8)
    prefix = args.output_dir / "oracle"
    subprocess.run(["pdftoppm", "-r", "72", "-singlefile", str(args.source), str(prefix)], check=True)
    header, pixels = prefix.with_suffix(".ppm").read_bytes().split(b"\n255\n", 1)
    height, width = actual.shape[:2]
    if header != f"P6\n{width} {height}".encode():
        raise ValueError("oracle dimensions differ")
    oracle = np.frombuffer(pixels, dtype=np.uint8).reshape(height, width, 3)
    errors = np.abs(actual.astype(np.int16) - oracle.astype(np.int16))
    (args.output_dir / "composited.ppm").write_bytes(header+b"\n255\n"+actual.tobytes())
    report = {"scope": "Specific 17-child leaf diagnostic; child interiors have opacity-one normal fills. Shape extracted by overriding enclosing child constant opacity only. Not a general PDF shape renderer or production fix.",
              "source_sha256": hashlib.sha256(args.source.read_bytes()).hexdigest(), "inputs": inputs,
              "different_pixels": int(np.count_nonzero(np.any(errors, axis=2))), "max_channel_error": int(errors.max()),
              "mean_absolute_channel_error": float(errors.mean()), "pixels_above_32": int(np.count_nonzero(errors.max(axis=2)>32))}
    args.report.write_text(json.dumps(report, indent=2)+"\n")
    print({k:v for k,v in report.items() if k != "inputs"})


if __name__ == "__main__":
    main()
