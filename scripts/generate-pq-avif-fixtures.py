"""CC0 synthetic 10-bit PQ/HLG AVIF fixtures. External FFmpeg/SVT-AV1 only.
Recreates grayscale and nonneutral producers plus independent RGBA16 readback.
Requires ffmpeg with libsvtav1; never invoked by the production application.
"""
from pathlib import Path
import struct
import subprocess
import tempfile

root = Path(__file__).resolve().parent.parent / "tests/fixtures/raster"
w, h = 128, 64
colors = [(65535, 0, 0), (0, 65535, 0), (0, 0, 65535), (32768, 49152, 16384)]
with tempfile.TemporaryDirectory(prefix="rrrah-pq-producer-") as directory:
    for transfer, prefix, trc in [(16, "pq", "smpte2084"), (18, "hlg", "arib-std-b67")]:
        for color in [False, True]:
            if color:
                samples = [colors[x // 32][channel] for channel in [1, 2, 0]
                           for y in range(h) for x in range(w)]
            else:
                samples = [(0, 128, 512, 1023)[x // 32] for y in range(h) for x in range(w)]
                samples += [512] * (w * h // 2)
            raw = Path(directory) / "input.raw"
            raw.write_bytes(struct.pack("<" + "H" * len(samples), *samples))
            name = f"avif-{prefix}10-color-svt.avif" if color else f"avif-{prefix}10-svt.avif"
            output = root / name
            command = ["ffmpeg", "-hide_banner", "-loglevel", "error", "-f", "rawvideo",
                       "-pixel_format", "gbrp16le" if color else "yuv420p10le",
                       "-video_size", "128x64", "-framerate", "1", "-color_range", "pc",
                       "-color_primaries", "bt2020", "-color_trc", trc,
                       "-colorspace", "rgb" if color else "bt2020nc", "-i", str(raw)]
            if color:
                command += ["-vf", "scale=out_color_matrix=bt2020:out_range=full", "-pix_fmt", "yuv420p10le"]
            command += ["-frames:v", "1", "-c:v", "libsvtav1", "-crf", "0", "-preset", "8",
                        "-svtav1-params", f"color-primaries=9:transfer-characteristics={transfer}:matrix-coefficients=9",
                        "-color_range", "pc", "-color_primaries", "bt2020", "-color_trc", trc,
                        "-colorspace", "bt2020nc", "-y", str(output)]
            subprocess.run(command, check=True)
            subprocess.run(["ffmpeg", "-hide_banner", "-loglevel", "error", "-i", str(output),
                            "-frames:v", "1", "-pix_fmt", "rgba64le", "-f", "rawvideo", "-y",
                            str(output) + ".rgba16le"], check=True)
