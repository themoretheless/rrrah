#!/usr/bin/env python3
"""Compare the three pinned AI pages from the test-only native compositor to Poppler."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess

PINS = {
    "VectorApple.ai": "c5fc34346629c3379cee6065c45b7de0b620be822604551d1463d8e691b03d06",
    "one.ai": "97786949c268233a446e1738fb9c2f941e4d109811bd2f972948cdb7a45d64b4",
}
PAGES = {("VectorApple.ai", 0), ("one.ai", 0), ("one.ai", 1)}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--native-dir", type=Path, required=True)
    parser.add_argument("--corpus", type=Path, required=True)
    parser.add_argument("--artifact-dir", type=Path, required=True,
                        help="Fresh, nonexistent directory for Poppler and native RGB planes")
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--poppler", default="pdftoppm")
    args = parser.parse_args()
    diagnostic_bytes = (args.native_dir / "report.json").read_bytes()
    rows = json.loads(diagnostic_bytes)
    assert len(rows) == len(PAGES) and {(r["fixture"], r["page"]) for r in rows} == PAGES
    for name, expected in PINS.items():
        assert digest((args.corpus / name).read_bytes()) == expected, f"unpinned source: {name}"
    for row in rows:
        assert row.get("native_completed") is True and row.get("capture_failed") is False
        assert row.get("deadline_expired") is False and row["final_admitted_bytes"] == 0
    args.artifact_dir.mkdir()
    version = subprocess.run([args.poppler, "-v"], capture_output=True,
                             text=True, check=True).stderr.strip()
    for name in PINS:
        subprocess.run([args.poppler, "-r", "72", str(args.corpus / name),
                        str(args.artifact_dir / name)], check=True)
    comparisons = []
    for row in rows:
        name, index = row["fixture"], row["page"]
        width, height = row["width"], row["height"]
        rgba = (args.native_dir / f"{name}-page-{index}.native.rgba").read_bytes()
        assert len(rgba) == width * height * 4
        original = (args.native_dir / f"{name}-page-{index}.native.components.f64le").read_bytes()
        channels = {"Gray": 1, "Rgb": 3, "Cmyk": 4}[row["page_model"]]
        assert len(original) == width * height * (channels + 1) * 8
        # Same explicit white composite as the independent Poppler RGB plane.
        actual = bytes((component * alpha + 255 * (255 - alpha) + 127) // 255
                       for offset in range(0, len(rgba), 4)
                       for alpha in [rgba[offset + 3]]
                       for component in rgba[offset:offset + 3])
        ppm = args.artifact_dir / f"{name}-{index + 1}.ppm"
        header, oracle = ppm.read_bytes().split(b"\n255\n", 1)
        assert header == f"P6\n{width} {height}".encode() and len(oracle) == len(actual)
        different = [i // 3 for i in range(0, len(actual), 3)
                     if actual[i:i + 3] != oracle[i:i + 3]]
        errors = [abs(a - b) for a, b in zip(actual, oracle)]
        bounds = ([min(i % width for i in different), min(i // width for i in different),
                   max(i % width for i in different), max(i // width for i in different)]
                  if different else None)
        (args.artifact_dir / f"{name}-{index}.native.ppm").write_bytes(
            f"P6\n{width} {height}\n255\n".encode() + actual)
        comparisons.append({"fixture": name, "page": index, "dimensions": [width, height],
                            "source_sha256": PINS[name], "native_rgba_sha256": digest(rgba),
                            "native_components_f64le_sha256": digest(original),
                            "poppler_rgb_sha256": digest(oracle),
                            "different_pixels": len(different), "max_channel_error": max(errors),
                            "mean_absolute_channel_error": sum(errors) / len(errors),
                            "difference_bounds_inclusive": bounds, "exact": not different})
    report = {"scope": "Strict 72 dpi RGB full-plane white-composite comparison; no tolerance",
              "poppler_version": version, "pages": comparisons, "native_diagnostic_pages": rows,
              "native_diagnostic_report_sha256": digest(diagnostic_bytes),
              "artifact_directory": str(args.artifact_dir.resolve()),
              "native_output_directory": str(args.native_dir.resolve()),
              "limitations": ["Test-only compositor; no production AI or Metal qualification",
                              "Non-RGB root preview uses explicit native device-to-RGB conversion",
                              "Color-management differences remain strict failures",
                              "Input artifacts must originate from the supplied pinned corpus"]}
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    for row in comparisons:
        print(f"{row['fixture']} page {row['page']}: different={row['different_pixels']} "
              f"max_error={row['max_channel_error']}")
    raise SystemExit(0 if all(row["exact"] for row in comparisons) else 1)


if __name__ == "__main__":
    main()
