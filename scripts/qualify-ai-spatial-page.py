#!/usr/bin/env python3
"""Strict RGB comparison of a completed VectorApple spatial diagnostic page."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import sys

SOURCE_SHA256 = "c5fc34346629c3379cee6065c45b7de0b620be822604551d1463d8e691b03d06"
DIMENSIONS = (301, 246)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def ppm(path):
    data = path.read_bytes()
    header = re.match(rb"P6\n(\d+) (\d+)\n255\n", data)
    if header is None:
        raise ValueError(f"unexpected diagnostic PPM header: {path}")
    dimensions = tuple(map(int, header.groups()))
    pixels = data[header.end():]
    if dimensions != DIMENSIONS or len(pixels) != dimensions[0] * dimensions[1] * 3:
        raise ValueError(f"incomplete or unexpected RGB plane: {path}")
    return pixels, digest(data)


def qualify(args):
    if digest(args.source.read_bytes()) != SOURCE_SHA256:
        raise ValueError("source is not the pinned VectorApple fixture")
    log_bytes = args.diagnostic_log.read_bytes()
    log = log_bytes.decode("utf-8")
    pages = re.findall(r"^REAL_AI_SPATIAL_PAGE width=(\d+) height=(\d+) completed=(\d+) total=(\d+) deadline=(\w+) peak_bytes=(\d+)$", log, re.M)
    if len(pages) != 1:
        raise ValueError("require exactly one terminal full-page result")
    width, height, completed, total = map(int, pages[0][:4])
    if (width, height) != DIMENSIONS or completed != total or total != width * height or pages[0][4] != "false":
        raise ValueError("page incomplete or deadline expired")
    if int(pages[0][5]) > 256 * 1024 * 1024:
        raise ValueError("page exceeded the ordinary memory allowance")
    if "REAL_AI_SPATIAL_PAGE_REFUSAL" in log or re.findall(r"^REAL_AI_SPATIAL_FINAL_CREDIT=(\d+)$", log, re.M) != ["0"]:
        raise ValueError("page refusal or unbalanced final admission")
    limits = re.findall(r"^REAL_AI_DIAGNOSTIC_PAGE_WORK_LIMIT=(\d+)$", log, re.M)
    if limits != ["4000000"] or not re.search(r"^test result: ok\. 1 passed; 0 failed;", log, re.M):
        raise ValueError("require a successful ordinary-budget diagnostic")
    maxima = re.findall(r"^REAL_AI_SPATIAL_PAGE_MAX_WORK spent=(\d+) pixel=\((\d+),(\d+)\) limit=(\d+)$", log, re.M)
    if len(maxima) != 1 or int(maxima[0][0]) > 4_000_000 or maxima[0][3] != "4000000":
        raise ValueError("missing or invalid full-page work evidence")
    native, native_file_hash = ppm(args.native)
    oracle, oracle_file_hash = ppm(args.oracle)
    metadata_bytes = args.oracle_metadata.read_bytes()
    metadata = json.loads(metadata_bytes)
    if metadata.get("source_sha256") != SOURCE_SHA256 or metadata.get("dpi") != 72 or metadata.get("page") != 0 or metadata.get("rgb_sha256") != digest(oracle):
        raise ValueError("oracle metadata does not match the source/RGB plane")
    errors = [abs(a - b) for a, b in zip(native, oracle)]
    different = [i // 3 for i in range(0, len(native), 3) if native[i:i + 3] != oracle[i:i + 3]]
    bounds = ([min(i % width for i in different), min(i // width for i in different),
               max(i % width for i in different), max(i // width for i in different)] if different else None)
    report = {"scope": "Strict supplied 72 dpi white-composited RGB planes; zero tolerance",
              "source_sha256": SOURCE_SHA256, "dimensions": [width, height],
              "diagnostic_log_sha256": digest(log_bytes), "native_ppm_sha256": native_file_hash,
              "oracle_ppm_sha256": oracle_file_hash, "oracle_metadata_sha256": digest(metadata_bytes),
              "oracle_version_recorded": metadata.get("poppler_version"),
              "different_pixels": len(different), "max_channel_error": max(errors),
              "mean_absolute_channel_error": sum(errors) / len(errors),
              "difference_bounds_inclusive": bounds, "exact": not different,
              "limitations": ["Test-only compositor; no production AI, Metal or 100-format qualification",
                              "Passing RGB comparison does not qualify other pages, camera formats or HDR"]}
    with args.report.open("x") as output:
        json.dump(report, output, indent=2)
        output.write("\n")
    print(f"different_pixels={len(different)} max_channel_error={max(errors)} exact={not different}")
    return 0 if not different else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("native", "diagnostic-log", "oracle", "oracle-metadata", "source", "report"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    try:
        return qualify(args)
    except (ValueError, OSError) as error:
        print(f"qualification refused: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
