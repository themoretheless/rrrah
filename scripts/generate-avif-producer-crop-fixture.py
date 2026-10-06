"""CC0 fixtures exported by external libavif, with independent Pillow pixel oracle.
Pass a compiled avif-crop-oracle executable; production code is not used.
"""
import hashlib
from pathlib import Path
import subprocess
import sys
from PIL import Image, ImageOps

root = Path(__file__).resolve().parent.parent / "tests/fixtures/raster"
tool = sys.argv[1]
sources = ["avif-rgb.avif"] + [f"avif-orientation-{i}.avif" for i in range(2, 9)]
manifest = root / "manifest.tsv"
lines = [line for line in manifest.read_text().splitlines()
         if not line.startswith("avif-libavif-crop-")]
for name in sources:
    output = "avif-libavif-crop-" + name.removeprefix("avif-")
    path = root / output
    subprocess.run([tool, "--export-top-left", str(root / name), str(path)], check=True)
    with Image.open(path) as image:
        # Pillow exposes stored pixels and orientation but does not apply clap.
        rectangle = (0, 0, image.width // 2, image.height // 2)
        expected = ImageOps.exif_transpose(image.convert("RGBA").crop(rectangle))
        (root / (output + ".rgba")).write_bytes(expected.tobytes())
        lines.append(f"{output}\t{expected.width}\t{expected.height}\t3\t"
                     f"{output}.rgba\t{hashlib.sha256(path.read_bytes()).hexdigest()}")
        print(output, image.size, rectangle, expected.size)
manifest.write_text("\n".join(lines) + "\n")
