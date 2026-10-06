#!/usr/bin/env python3
"""Independent LittleCMS float-CMYK diagnostic for the pinned opaque PDF strips.
Production Hayro DeviceCMYK quantizes to u8 before ICC. This tool measures
direct float input as well as an external reproduction of that quantization.
It does not relax the strict AI/Poppler rendering gate.
"""
import argparse
import ctypes as c
import hashlib
import json
from pathlib import Path
import re
import struct

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--profile", type=Path, required=True)
parser.add_argument("--native", type=Path, required=True)
parser.add_argument("--library", type=Path, required=True)
parser.add_argument("--report", type=Path, required=True)
args = parser.parse_args()
root = Path(__file__).resolve().parents[1]
fixture = root / "tests/fixtures/pdf/gradient-center-strips-cmyk.pdf"
digest = lambda data: hashlib.sha256(data).hexdigest()
source = fixture.read_bytes()
assert digest(source) == "20e32ad2763eeba1c4c01864f2a00822f6e56811a6173379f99231f1637d16e2"
profile = args.profile.read_bytes()
assert digest(profile) == "73e1ba37d2bad5bab2a964f40a9eed96209666efc067c3322626214bbef234a0"
native = args.native.read_bytes()
assert len(native) == 12 * 8 * 4 and all(native[i] == 255 for i in range(3, len(native), 4))
actual = bytes(v for i, v in enumerate(native) if i % 4 != 3)
f32 = lambda v: struct.unpack("f", struct.pack("f", v))[0]
colors = [[f32(float(token)) for token in tokens] for tokens in
          re.findall(rb"([0-9.]+) ([0-9.]+) ([0-9.]+) ([0-9.]+) k ", source)]
assert len(colors) == 12
# LittleCMS float CMYK uses percentage units; normalized PDF samples use 0–1.
float_input = (c.c_float * (96 * 4))(*[f32(v * 100) for row in colors * 8 for v in row])
quantized_input = (c.c_ubyte * (96 * 4))(*[int(f32(f32(v * 255) + 0.5)) for row in colors * 8 for v in row])
lcms = c.CDLL(str(args.library.resolve()))
def bind(name, result, *arguments):
    function = getattr(lcms, name)
    function.restype, function.argtypes = result, list(arguments)
    return function
version = bind("cmsGetEncodedCMMversion", c.c_int)()
open_profile = bind("cmsOpenProfileFromMem", c.c_void_p, c.c_void_p, c.c_uint32)
create_srgb = bind("cmsCreate_sRGBProfile", c.c_void_p)
create_transform = bind("cmsCreateTransform", c.c_void_p, c.c_void_p, c.c_uint32,
                        c.c_void_p, c.c_uint32, c.c_uint32, c.c_uint32)
transform = bind("cmsDoTransform", None, c.c_void_p, c.c_void_p, c.c_void_p, c.c_uint32)
delete = bind("cmsDeleteTransform", None, c.c_void_p)
close = bind("cmsCloseProfile", c.c_int, c.c_void_p)
profile_buffer = c.create_string_buffer(profile)
source_profile = open_profile(profile_buffer, len(profile))
destination = create_srgb()
assert source_profile and destination
rows = []
try:
    # Format identifiers derive from public lcms2.h macros, not private ABI.
    rgb8 = (4 << 16) | (3 << 3) | 1
    for intent in range(4):
        for mode, samples, format_code in [
            ("float_cmyk_percent", float_input, (1 << 22) | (6 << 16) | (4 << 3) | 4),
            ("hayro_quantized_cmyk8", quantized_input, (6 << 16) | (4 << 3) | 1),
        ]:
            executor = create_transform(source_profile, format_code, destination, rgb8, intent, 0)
            assert executor, "LittleCMS refused the requested transform"
            output = (c.c_ubyte * (96 * 3))()
            try:
                transform(executor, samples, output, 96)
            finally:
                delete(executor)
            reference = bytes(output)
            errors = [abs(a - b) for a, b in zip(actual, reference)]
            rows.append({"intent": intent, "input_mode": mode,
                         "max_channel_difference": max(errors),
                         "different_pixels": sum(actual[i:i+3] != reference[i:i+3] for i in range(0, len(actual), 3)),
                         "reference_first_row_hex": reference[:36].hex()})
finally:
    close(destination)
    close(source_profile)
report = {"fixture_sha256": digest(source), "profile_sha256": digest(profile),
          "native_sha256": digest(native), "littlecms_encoded_version": version,
          "library_sha256": digest(args.library.read_bytes()), "normalized_f32_cmyk": colors,
          "results": rows,
          "limitations": ["One opaque strip fixture and identical ICC bytes; RGB output is quantized to u8.",
                          "Hayro 0.7.0 DeviceCMYK quantizes float input to u8 before ICC conversion.",
                          "Does not qualify all PDF/AI colors, rendering intents, masks, blends or PostScript.",
                          "Strict Poppler AI gate remains unchanged."]}
args.report.write_text(json.dumps(report, indent=2) + "\n")
print(json.dumps(rows))
