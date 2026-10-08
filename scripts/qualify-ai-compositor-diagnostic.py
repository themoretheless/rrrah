#!/usr/bin/env python3
"""Compare bounded test-compositor replay of pinned real AI pages with Poppler.

This diagnoses the experimental compositor, never qualifies production decoding.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("corpus", type=Path)
    parser.add_argument("replay", type=Path)
    parser.add_argument("--report", required=True, type=Path)
    args = parser.parse_args()
    project = Path(__file__).resolve().parents[1]
    manifest = json.loads((project / "docs/research/ai-upstream-poppler-2026-10-06.json").read_text())
    sources = {source["name"]: source for source in manifest["sources"]}
    for name, source in sources.items():
        data = (args.corpus / name).read_bytes()
        if len(data) != source["bytes"] or hashlib.sha256(data).hexdigest() != source["sha256"]:
            raise ValueError(f"pinned source mismatch: {name}")
    rows = json.loads((args.replay / "report.json").read_text())
    required = set()
    for name in sources:
        info = subprocess.check_output(["pdfinfo", str((args.corpus/name).resolve())], text=True)
        count = int(next(line.split(":",1)[1] for line in info.splitlines() if line.startswith("Pages:")))
        required.update((name,page) for page in range(count))
    observed = [(row["fixture"],row["page"]) for row in rows]
    if len(observed) != len(required) or set(observed) != required:
        raise ValueError("every pinned page must be represented exactly once")
    oracle_dir = args.replay / "poppler"
    oracle_dir.mkdir(exist_ok=False)
    for row in rows:
        name, page = row["fixture"], row["page"]
        if name not in sources:
            raise ValueError("unknown source")
        row["source_sha256"] = sources[name]["sha256"]
        row["exact"] = False
        if row["status"] != "replayed":
            continue
        prefix = oracle_dir / f"{name}-page-{page}"
        subprocess.run(["pdftoppm", "-r", "72", "-f", str(page+1), "-l", str(page+1),
                        "-singlefile", str((args.corpus/name).resolve()), str(prefix.resolve())], check=True)
        header, expected = Path(str(prefix)+".ppm").read_bytes().split(b"\n255\n", 1)
        width, height = row["width"], row["height"]
        if header != f"P6\n{width} {height}".encode():
            raise ValueError("dimension mismatch")
        rgba = (args.replay/f"{name}-page-{page}.rgba").read_bytes()
        if len(rgba) != width*height*4 or len(expected) != width*height*3:
            raise ValueError("payload length mismatch")
        if any(alpha != 255 for alpha in rgba[3::4]):
            raise ValueError("white-backdrop replay must be opaque")
        actual = bytes(value for pixel in range(width*height) for value in rgba[pixel*4:pixel*4+3])
        errors = [abs(a-b) for a,b in zip(actual,expected)]
        different = sum(any(errors[i:i+3]) for i in range(0,len(errors),3))
        row.update(different_pixels=different,max_channel_error=max(errors),
                   mean_absolute_error=sum(errors)/len(errors),exact=different==0,
                   rgba_sha256=hashlib.sha256(rgba).hexdigest(),
                   oracle_rgb_sha256=hashlib.sha256(expected).hexdigest())
        if "tile_edge" in row:
            tiled_path = args.replay/f"{name}-page-{page}.tile-{row['tile_edge']}.rgba"
            if tiled_path.exists():
                tiled = tiled_path.read_bytes()
                if len(tiled) != len(rgba):
                    raise ValueError("tile payload length mismatch")
                cases = [{"x":(i//4)%width,"y":(i//4)//width,
                          "whole":list(rgba[i:i+4]),"tile":list(tiled[i:i+4])}
                         for i in range(0,len(rgba),4) if rgba[i:i+4] != tiled[i:i+4]]
                if len(cases) != row["tile_different_pixels"]:
                    raise ValueError("tile comparison metadata mismatch")
                row.update(tile_exact=not cases,tile_max_channel_error=max(abs(a-b) for a,b in zip(rgba,tiled)),
                           tile_difference_examples=cases[:100],tile_rgba_sha256=hashlib.sha256(tiled).hexdigest())
            else:
                row["tile_exact"] = False

    version = subprocess.run(["pdftoppm", "-v"], capture_output=True, text=True, check=True)
    report = {"scope":"Test-only compositor real-AI diagnostic; no production qualification",
              "poppler_version":(version.stdout+version.stderr).splitlines()[0],
              "rows":rows,"all_pages_exact":all(row["exact"] for row in rows),
              "all_tiled_pages_match":all(row.get("tile_exact",False) for row in rows)
                  if any("tile_edge" in row for row in rows) else None}
    args.report.write_text(json.dumps(report,indent=2)+"\n")
    raise SystemExit(0 if report["all_pages_exact"] else 1)


if __name__ == "__main__":
    main()
