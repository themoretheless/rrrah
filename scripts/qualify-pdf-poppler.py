#!/usr/bin/env python3
"""Strict full-plane PDF/Poppler comparison; saves failures and exits nonzero."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dump", required=True, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument("--poppler", default="pdftoppm")
    args = parser.parse_args()
    project = Path(__file__).resolve().parents[1]
    rows = []
    with tempfile.TemporaryDirectory(prefix="rrrah-pdf-compare-") as tmp:
        for name in ["rect", "circle", "triangle", "text"]:
            source = project / f"tests/fixtures/pdf/cairo-{name}.pdf"
            prefix = Path(tmp) / name
            native = prefix.with_suffix(".rgba")
            metadata = json.loads(subprocess.check_output(
                [str(args.dump.resolve()), str(source), "0", str(native)], text=True))
            subprocess.run([args.poppler, "-r", "72", str(source), str(prefix)], check=True)
            ppm = Path(str(prefix) + "-1.ppm").read_bytes()
            header, oracle = ppm.split(b"\n255\n", 1)
            width, height = metadata["width"], metadata["height"]
            assert header == f"P6\n{width} {height}".encode()
            rgba = native.read_bytes()
            assert len(rgba) == width * height * 4 and len(oracle) == width * height * 3
            actual = bytes((c*a + 255*(255-a) + 127)//255
                for r,g,b,a in zip(rgba[0::4],rgba[1::4],rgba[2::4],rgba[3::4])
                for c in (r,g,b))
            different = sum(actual[i:i+3] != oracle[i:i+3] for i in range(0,len(oracle),3))
            rows.append({"component":name,"source_sha256":hashlib.sha256(source.read_bytes()).hexdigest(),
                "dimensions":[width,height],"different_pixels":different,
                "max_channel_error":max(abs(a-b) for a,b in zip(actual,oracle)),"exact":different==0})
    args.report.write_text(json.dumps({"scope":"strict RGB white-composite comparison; no tolerance", "rows":rows},indent=2)+"\n")
    raise SystemExit(0 if all(row["exact"] for row in rows) else 1)


if __name__ == "__main__":
    main()
