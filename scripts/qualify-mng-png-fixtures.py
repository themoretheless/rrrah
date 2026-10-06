#!/usr/bin/env python3
"""Verify embedded PNG samples using FFmpeg; not outer MNG semantics qualification."""
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1] / "tests/fixtures/mng"


def main():
    manifest = json.loads((ROOT / "manifest.json").read_text())
    checks = []
    with tempfile.TemporaryDirectory(prefix="rrrah-mng-png-oracles-") as temporary:
        output = Path(temporary)
        for case in manifest["cases"]:
            source = (ROOT / case["file"]).read_bytes()
            assert hashlib.sha256(source).hexdigest() == case["sha256"]
            offset, start, index = 8, None, 0
            decoded_layers = []
            while offset < len(source):
                size = int.from_bytes(source[offset:offset + 4], "big")
                kind = source[offset + 4:offset + 8]
                end = offset + size + 12
                assert end <= len(source)
                if kind == b"IHDR":
                    start = offset
                if kind == b"IEND":
                    png = output / "frame.png"
                    png.write_bytes(b"\x89PNG\r\n\x1a\n" + source[start:end])
                    raw = output / "frame.raw"
                    fmt = "rgba" if case["sample_bits"] == 8 else "rgba64le"
                    subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", str(png),
                                    "-frames:v", "1", "-pix_fmt", fmt, "-f", "rawvideo", str(raw)], check=True)
                    data = raw.read_bytes()
                    values = list(data) if case["sample_bits"] == 8 else list(
                        struct.unpack("<" + str(len(data) // 2) + "H", data))
                    assert values == case.get("layer_rgba", case["rgba"])[index], (case["file"], index)
                    decoded_layers.append(values)
                    target = ROOT / f"{case['file']}-{index}.rgba"
                    target.write_bytes(data)
                    checks.append({"file": case["file"], "embedded_image": index,
                                   "png_samples_match_authored_values": True, "rgba_file": target.name,
                                   "rgba_sha256": hashlib.sha256(data).hexdigest()})
                    index += 1
                offset = end
            assert index == len(case.get("layer_rgba", case["rgba"]))
            canvas = [0] * (case["width"]*case["height"]*4)
            presentations = []
            for values,(width,height) in zip(decoded_layers,case["layer_dimensions"]):
                for row in range(height):
                    at=row*case["width"]*4
                    canvas[at:at+width*4]=values[row*width*4:(row+1)*width*4]
                if case["ticks_per_second"]: presentations.append(canvas.copy())
            if not case["ticks_per_second"]: presentations=[canvas.copy()]
            assert presentations == case["rgba"], case["file"]
            for frame,values in enumerate(presentations):
                target=ROOT/f"{case['file']}-frame-{frame}.rgba"
                target.write_bytes(bytes(values) if case["sample_bits"] == 8 else
                                   struct.pack("<"+str(len(values))+"H",*values))

    report = {
        "scope": "Independent FFmpeg decoding of each extracted standalone PNG; not external MNG composition/timing qualification",
        "ffmpeg_version": subprocess.check_output(["ffmpeg", "-version"], text=True).splitlines()[0],
        "checks": checks,
    }
    (ROOT / "png-oracle-qualification.json").write_text(json.dumps(report, indent=2) + "\n")
    print(f"Verified {len(checks)} embedded PNG planes")


if __name__ == "__main__":
    main()
