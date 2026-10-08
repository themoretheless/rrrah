#!/usr/bin/env python3
"""Render paired authored PDF groups differing only in blending color space.

Poppler is a test oracle; this never qualifies the product renderer.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess


def document(space):
    def stream(dictionary, content):
        return f"<< {dictionary} /Length {len(content)} >>\nstream\n{content}\nendstream".encode()
    paint = "q /half gs 1 0 0 0 k 0 0 16 16 re f Q\nq /half gs 0 1 0 0 k 0 0 16 16 re f Q"
    objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 16 16] /Resources << /XObject << /G 6 0 R >> >> /Contents 4 0 R >>",
        stream("", "/G Do"),
        b"<< /Type /ExtGState /ca 0.5 /BM /Normal >>",
        stream(f"/Type /XObject /Subtype /Form /BBox [0 0 16 16] /Group << /S /Transparency /I true /K false /CS /{space} >> /Resources << /ExtGState << /half 5 0 R >> >>", paint),
    ]
    data = bytearray(b"%PDF-1.4\n")
    offsets = [0]
    for i, obj in enumerate(objects, 1):
        offsets.append(len(data))
        data.extend(f"{i} 0 obj\n".encode() + obj + b"\nendobj\n")
    xref = len(data)
    data.extend(f"xref\n0 {len(offsets)}\n0000000000 65535 f \n".encode())
    for offset in offsets[1:]:
        data.extend(f"{offset:010d} 00000 n \n".encode())
    data.extend(f"trailer\n<< /Size {len(offsets)} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode())
    return bytes(data)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path, help="Fresh output directory")
    args = parser.parse_args()
    args.output.mkdir(exist_ok=False)
    rows = []
    for space in ["DeviceRGB", "DeviceCMYK"]:
        data = document(space)
        pdf = args.output / f"{space}.pdf"
        pdf.write_bytes(data)
        prefix = args.output / space
        subprocess.run(["pdftoppm", "-r", "72", "-singlefile", str(pdf), str(prefix)], check=True)
        header, pixels = prefix.with_suffix(".ppm").read_bytes().split(b"\n255\n", 1)
        if header != b"P6\n16 16" or len(pixels) != 16 * 16 * 3:
            raise ValueError("Unexpected oracle output")
        index = (8 * 16 + 8) * 3
        rows.append({"group_space":space,"pdf_sha256":hashlib.sha256(data).hexdigest(),
                     "interior_rgb":list(pixels[index:index+3]),
                     "rgb_sha256":hashlib.sha256(pixels).hexdigest()})
    version = subprocess.run(["pdftoppm", "-v"], capture_output=True, text=True, check=True)
    report = {"scope":"Authored group-color-space test oracle, no product qualification",
              "oracle":(version.stdout+version.stderr).splitlines()[0],
              "only_changed_semantic_field":"Transparency group /CS",
              "paint":"Two overlapping native CMYK fills, opacity0.5, isolated normal group",
              "rows":rows,"distinct_interior_colors":rows[0]["interior_rgb"] != rows[1]["interior_rgb"]}
    (args.output/"report.json").write_text(json.dumps(report,indent=2)+"\n")
    # A backend may render both declared group spaces identically. Record that
    # observation; never treat it as proof that group-space propagation is correct.


if __name__ == "__main__":
    main()
