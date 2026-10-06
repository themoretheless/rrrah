#!/usr/bin/env python3
"""Compare pinned external AI pages with Poppler; exit 1 on any discrepancy."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import urllib.request


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("corpus", type=Path)
    parser.add_argument("--dump", required=True, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument("--fetch", action="store_true")
    parser.add_argument("--viewer-target-dir", type=Path,
                        help="Also run pinned-corpus viewer RAM and swap/GPU tests")
    args = parser.parse_args()
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
        for source in manifest["sources"]:
            path = (args.corpus / source["name"]).resolve()
            info = subprocess.check_output(["pdfinfo", str(path)], text=True)
            count = int(next(line.split(":", 1)[1] for line in info.splitlines()
                             if line.startswith("Pages:")))
            for index in range(count):
                row = {"file": path.name, "index": index, "exact": False}
                prefix = Path(tmp) / f"{path.stem}-{index}"
                native = prefix.with_suffix(".rgba")
                try:
                    metadata = json.loads(subprocess.check_output(
                        [str(args.dump.resolve()), str(path), str(index), str(native)],
                        text=True, stderr=subprocess.PIPE))
                    row["native"] = metadata
                    if metadata["image_index"] != index or metadata["image_count"] != count:
                        raise ValueError("selected page metadata mismatch")
                    subprocess.run(["pdftoppm", "-r", "72", "-f", str(index + 1),
                                    "-l", str(index + 1), "-singlefile", str(path), str(prefix)],
                                   check=True, capture_output=True)
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
                    row.update(different_pixels=different,
                               max_channel_error=max(abs(a-b) for a, b in zip(actual, oracle)),
                               rgba_sha256=sha256(rgba), exact=different == 0)
                except (ValueError, subprocess.CalledProcessError) as error:
                    row["error"] = str(error)
                rows.append(row)
    report = {"revision": manifest["revision"], "sources": manifest["sources"],
              "decoder_sha256": sha256(args.dump.read_bytes()),
              "poppler_version": subprocess.run(["pdftoppm", "-v"], capture_output=True,
                                                 text=True, check=True).stderr.strip(),
              "scope": "Strict 72 dpi white-composite RGB comparison; no tolerance. No GPU, private AI data or full format qualification.",
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
