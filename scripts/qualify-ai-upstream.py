#!/usr/bin/env python3
"""Compare pinned external AI pages with Poppler; exit 1 on any discrepancy."""
import argparse
import hashlib
import json
import os
import re
from pathlib import Path
import subprocess
import tempfile
import urllib.request


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def freeze_decoder(source, destination):
    # Execute the exact bytes hashed for this run. A concurrent build may
    # replace the source path while pages are being rendered.
    data = source.read_bytes()
    destination.write_bytes(data)
    destination.chmod(0o500)
    return sha256(data)


def stderr_text(detail):
    if isinstance(detail, bytes):
        return detail.decode("utf8", "replace")
    return detail if isinstance(detail, str) else ""


def attach_renderer_diagnostics(row, detail):
    detail = stderr_text(detail)
    if not detail:
        return
    row["renderer_stderr_tail"] = detail[-16384:]
    outcomes = []
    truncated = False
    pattern = (r"ORDERED_SAMPLE_OUTCOME patches=([0-9]{1,20}) complete=(true|false) "
               r"work_spent=([0-9]{1,20}) work_left=([0-9]{1,20})")
    for line in detail.splitlines():
        match = re.fullmatch(pattern, line)
        if match:
            if len(outcomes) == 64:
                truncated = True
                continue
            outcomes.append({"patches": int(match[1]), "complete": match[2] == "true",
                             "work_spent": int(match[3]), "work_left": int(match[4])})
    if outcomes:
        row["renderer_outcomes"] = outcomes
    if truncated:
        row["renderer_outcomes_truncated"] = True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("corpus", type=Path)
    parser.add_argument("--dump", required=True, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument("--fetch", action="store_true")
    parser.add_argument("--render-timeout", type=float, default=300.,
                        help="Maximum seconds per native or Poppler page render")
    parser.add_argument("--viewer-target-dir", type=Path,
                        help="Also run pinned-corpus viewer RAM and swap/GPU tests")
    args = parser.parse_args()
    if not 0. < args.render_timeout < float("inf"):
        parser.error("--render-timeout must be finite and positive")
    project = Path(__file__).resolve().parents[1]
    manifest = json.loads((project / "docs/research/ai-upstream-poppler-2026-10-06.json").read_text())
    args.corpus.mkdir(parents=True, exist_ok=True)
    # Verify every source before invoking either renderer. External artwork stays
    # outside the checkout; fetch only the pinned URLs from the source manifest.
    for source in manifest["sources"]:
        path = args.corpus / source["name"]
        if args.fetch and not path.exists():
            data = urllib.request.urlopen(source["source"], timeout=60).read()
            if len(data) != source["bytes"] or sha256(data) != source["sha256"]:
                raise ValueError(f"download hash mismatch: {path.name}")
            path.write_bytes(data)
        data = path.read_bytes()
        if len(data) != source["bytes"] or sha256(data) != source["sha256"]:
            raise ValueError(f"source hash mismatch: {path.name}")
    rows = []
    with tempfile.TemporaryDirectory(prefix="rrrah-ai-compare-") as tmp:
        decoder = Path(tmp) / "raster_fixture_dump"
        decoder_sha256 = freeze_decoder(args.dump.resolve(), decoder)
        for source in manifest["sources"]:
            path = (args.corpus / source["name"]).resolve()
            info = subprocess.check_output(["pdfinfo", str(path)], text=True)
            count = int(next(line.split(":", 1)[1] for line in info.splitlines()
                             if line.startswith("Pages:")))
            for index in range(count):
                row = {"file": path.name, "index": index, "exact": False}
                prefix = Path(tmp) / f"{path.stem}-{index}"
                native = prefix.with_suffix(".rgba")
                native_diagnostics_recorded = False
                try:
                    rendered = subprocess.run(
                        [str(decoder), str(path), str(index), str(native)],
                        text=True, capture_output=True, timeout=args.render_timeout)
                    attach_renderer_diagnostics(row, rendered.stderr)
                    native_diagnostics_recorded = True
                    rendered.check_returncode()
                    metadata = json.loads(rendered.stdout)
                    row["native"] = metadata
                    if metadata["image_index"] != index or metadata["image_count"] != count:
                        raise ValueError("selected page metadata mismatch")
                    subprocess.run(["pdftoppm", "-r", "72", "-f", str(index + 1),
                                    "-l", str(index + 1), "-singlefile", str(path), str(prefix)],
                                   check=True, capture_output=True, timeout=args.render_timeout)
                    header, oracle = prefix.with_suffix(".ppm").read_bytes().split(b"\n255\n", 1)
                    width, height = metadata["width"], metadata["height"]
                    row["poppler_header"] = header.decode("ascii")
                    if header != f"P6\n{width} {height}".encode():
                        raise ValueError("render dimensions mismatch")
                    rgba = native.read_bytes()
                    if len(rgba) != width * height * 4 or len(oracle) != width * height * 3:
                        raise ValueError("pixel payload length mismatch")
                    actual = bytes((c * a + 255 * (255 - a) + 127) // 255
                                   for r, g, b, a in zip(rgba[0::4], rgba[1::4], rgba[2::4], rgba[3::4])
                                   for c in (r, g, b))
                    different = sum(actual[i:i+3] != oracle[i:i+3]
                                    for i in range(0, len(oracle), 3))
                    absolute_sum = 0
                    squared_sum = 0
                    maximum = 0
                    above = {1: 0, 8: 0, 32: 0, 128: 0}
                    bounds = [width, height, -1, -1]
                    regions = {}
                    for offset in range(0, len(oracle), 3):
                        pixel_error = 0
                        for channel in range(3):
                            error = abs(actual[offset + channel] - oracle[offset + channel])
                            absolute_sum += error
                            squared_sum += error * error
                            maximum = max(maximum, error)
                            pixel_error = max(pixel_error, error)
                        if pixel_error:
                            pixel = offset // 3
                            x, y = pixel % width, pixel // width
                            bounds = [min(bounds[0], x), min(bounds[1], y), max(bounds[2], x), max(bounds[3], y)]
                            region = (x // 64, y // 64)
                            entry = regions.setdefault(region, [0, 0, 0])
                            entry[0] += 1
                            entry[1] += pixel_error
                            entry[2] = max(entry[2], pixel_error)
                        for threshold in above:
                            above[threshold] += pixel_error > threshold
                    row.update(different_pixels=different,
                               max_channel_error=maximum,
                               difference_bounds_inclusive=bounds if different else None,
                               highest_error_regions=[{"origin": [x * 64, y * 64], "different_pixels": values[0], "summed_max_channel_error": values[1], "max_channel_error": values[2]} for (x, y), values in sorted(regions.items(), key=lambda entry: (-entry[1][1], entry[0]))[:16]],
                               mean_absolute_channel_error=absolute_sum / len(oracle),
                               rms_channel_error=(squared_sum / len(oracle)) ** 0.5,
                               pixels_above_channel_error={str(key): value for key, value in above.items()},
                               rgba_sha256=sha256(rgba), exact=different == 0)
                except (ValueError, subprocess.CalledProcessError, subprocess.TimeoutExpired) as error:
                    row["error"] = str(error)
                    if isinstance(error, (subprocess.CalledProcessError, subprocess.TimeoutExpired)):
                        if not native_diagnostics_recorded:
                            attach_renderer_diagnostics(row, error.stderr)
                        elif "native" in row and error.stderr:
                            row["reference_stderr_tail"] = stderr_text(error.stderr)[-16384:]
                rows.append(row)
    report = {"revision": manifest["revision"], "sources": manifest["sources"],
              "decoder_sha256": decoder_sha256,
              "decoder_execution": "Private frozen copy; hash taken before any page render",
              "poppler_version": subprocess.run(["pdftoppm", "-v"], capture_output=True,
                                                 text=True, check=True).stderr.strip(),
              "scope": "Strict 72 dpi white-composite RGB comparison; no tolerance. No GPU, private AI data or full format qualification.",
              "render_timeout_seconds": args.render_timeout,
              "pages": rows}
    viewer_results = []
    if args.viewer_target_dir is not None:
        environment = dict(os.environ, RRRAH_AI_CORPUS=str(args.corpus.resolve()))
        for target, test in [
            (["--bin", "rrrah"], "foreground_loader_tests::real_ai_pages_use_shared_ram_hits_and_independent_prefetch_keys"),
            (["--test", "pdf_readback"], "real_ai_pages_swap_preserves_pixels_and_metal_frame"),
        ]:
            result = subprocess.run(
                ["cargo", "test", "-p", "rrrah", *target, test, "--locked",
                 "--target-dir", str(args.viewer_target_dir.resolve()),
                 "--", "--ignored", "--exact", "--nocapture"],
                cwd=project, env=environment, capture_output=True, text=True)
            # Cargo can succeed with zero selected tests; require the named test.
            output = result.stdout + result.stderr
            passed = result.returncode == 0 and f"test {test} ... ok" in output
            viewer_results.append({"test": test, "exit": result.returncode,
                                   "passed": passed, "output": output})
        report["viewer_tests"] = viewer_results
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    passed = rows and all(row["exact"] for row in rows)
    passed = passed and all(row["passed"] for row in viewer_results)
    raise SystemExit(0 if passed else 1)


if __name__ == "__main__":
    main()
