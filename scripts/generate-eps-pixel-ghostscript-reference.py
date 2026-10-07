#!/usr/bin/env python3
"""Authored PostScript fixtures, rendered by test-only Ghostscript via stdin."""
import hashlib
import json
import subprocess
import sys
from pathlib import Path

box = "1 1 moveto 7 1 lineto 7 7 lineto 1 7 lineto closepath"
hole = "3 3 moveto 5 3 lineto 5 5 lineto 3 5 lineto closepath"
cases = [
    ("red_rectangle", "1 0 0 setrgbcolor " + box + " fill"),
    ("same_winding_nonzero", box + " " + hole + " fill"),
    ("same_winding_evenodd", box + " " + hole + " eofill"),
    ("affine_rectangle", "1 2 translate 2 3 scale 0 0 moveto 2 0 lineto 2 1 lineto 0 1 lineto closepath fill"),
    ("ordered_layers", "1 0 0 setrgbcolor " + box + " fill 0 0 1 setrgbcolor " + hole + " fill"),
    ("saved_fill_path", box + " gsave 0.5 setgray fill grestore " + hole + " eofill"),
    ("gray_quarter", "0.25 setgray " + box + " fill"),
    ("gray_three_quarters", "0.75 setgray " + box + " fill"),
    ("rgb_half", "0.5 0.5 0.5 setrgbcolor " + box + " fill"),
]
bevel_outlines = "--bevel-outlines" in sys.argv
bevel_boundaries = "--bevel-boundaries" in sys.argv or bevel_outlines
stroke = "--stroke" in sys.argv or bevel_boundaries
color_ramp = "--color-ramp" in sys.argv
if color_ramp:
    if any(flag in sys.argv for flag in ("--stroke", "--eps-import")):
        raise SystemExit("--color-ramp cannot be combined with stroke/import modes")
    values = sorted(set([i / 32 for i in range(33)] + [0.5 - 1 / 65536, 0.5 + 1 / 65536]))
    cases = [(f"{kind}_{value:.12g}", f"{value:.12g} setgray " + box + " fill" if kind == "gray" else f"{value:.12g} {value:.12g} {value:.12g} setrgbcolor " + box + " fill") for kind in ("gray", "rgb") for value in values]
if stroke:
    cases = [
        ("butt", "2 setlinewidth 0 setlinecap 2 4 moveto 6 4 lineto stroke"),
        ("square", "2 setlinewidth 2 setlinecap 2 4 moveto 6 4 lineto stroke"),
        ("miter", "2 setlinewidth 0 setlinejoin 2 2 moveto 5 2 lineto 5 6 lineto stroke"),
        ("bevel", "2 setlinewidth 2 setlinejoin 2 2 moveto 5 2 lineto 5 6 lineto stroke"),
        ("round", "2 setlinewidth 1 setlinejoin 2 2 moveto 5 2 lineto 5 6 lineto stroke"),
        ("limited_miter", "2 setlinewidth 0 setlinejoin 1 setmiterlimit 2 2 moveto 5 2 lineto 5 6 lineto stroke"),
    ]
eps_import = "--eps-import" in sys.argv
if bevel_boundaries:
    if eps_import or color_ramp:
        raise SystemExit("bevel boundary mode must be used alone")
    cases = [(f"bevel_shift_{dx}", f"{dx} 0 translate 2 setlinewidth 2 setlinejoin 2 2 moveto 5 2 lineto 5 6 lineto stroke") for dx in (-0.03125, -0.015625, 0.015625, 0.03125)]
    if bevel_outlines:
        outline = "2 1 moveto 5 1 lineto 5 3 lineto 2 3 lineto closepath 4 2 moveto 6 2 lineto 6 6 lineto 4 6 lineto closepath 5 2 moveto 5 1 lineto 6 2 lineto closepath fill"
        cases = [(f"{kind}_{dx}", f"{dx} 0 translate " + program) for dx in (-0.03125, -0.015625, 0, 0.015625, 0.03125) for kind, program in (("stroke", "2 setlinewidth 2 setlinejoin 2 2 moveto 5 2 lineto 5 6 lineto stroke"), ("outline", outline))]
if eps_import:
    if stroke:
        raise SystemExit("--eps-import and --stroke are mutually exclusive")
    cases = [
        ("terminal_showpage_rectangle", "1 0 0 setrgbcolor 1 1 6 6 rectfill showpage"),
        ("computed_negative_rectangle_showpage_alias", "1 0 0 setrgbcolor 49 sqrt 7 -6 -6 rectfill showpage /flush /showpage load def flush 0 0 1 setrgbcolor 7 2 idiv 3 2 2 rectfill flush"),
        ("logarithmic_rectangle_coordinates", "1 0 0 setrgbcolor 100 log 1 ln 1000 log 16 sqrt rectfill 0 0 1 setrgbcolor 10 log 100 log 4 4 rectfill showpage"),
        ("arctangent_rotation_numeric_conversion", "1 0 0 setrgbcolor 7.9 cvi 1 cvr translate 1 0 atan rotate 0 0 4.9 cvi 4 cvr rectfill showpage"),
        ("power_computed_rectangle", "1 0 0 setrgbcolor 2 -1 exp 2 mul 1 2 2 exp 2 2 exp rectfill 0 0 1 setrgbcolor 3 3 9 0.5 exp 2 1 exp rectfill showpage"),
    ]
background = "1 setgray 0 0 moveto 8 0 lineto 8 8 lineto 0 8 lineto closepath fill 0 setgray "
unadjusted = stroke and ("--no-stroke-adjust" in sys.argv or bevel_boundaries)
prefix = "false setstrokeadjust " if unadjusted else ""
zero_fill_adjust = "--zero-fill-adjust" in sys.argv
if zero_fill_adjust:
    if not bevel_boundaries:
        raise SystemExit("zero fill adjustment currently qualifies bevel modes only")
    prefix += "0 0 .setfilladjust2 "
source = "\n".join(background + prefix + program + (" rrrahEmit" if eps_import else " showpage") for _, program in cases).encode()
if eps_import:
    source = b"/rrrahEmit /showpage load def /showpage {} def\n" + source
result = subprocess.run([
    "docker", "run", "--rm", "-i", "alpine:3.22.1", "sh", "-c",
    "apk add --no-cache ghostscript >/tmp/install.log && gs --version >&2 && sha256sum /usr/bin/gs >&2 && gs -q -dBATCH -dNOPAUSE -sDEVICE=ppmraw -dGraphicsAlphaBits=1 -r72 -g8x8 -sOutputFile=%stdout -"
], input=source, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=120, check=True)
data = result.stdout
at = 0
def token():
    global at
    while True:
        while data[at] in b" \t\r\n": at += 1
        if data[at] != 35: break
        at = data.index(b"\n", at) + 1
    begin = at
    while data[at] not in b" \t\r\n": at += 1
    return data[begin:at]
references = []
for name, program in cases:
    assert token() == b"P6"
    assert (token(), token(), token()) == (b"8", b"8", b"255")
    assert data[at] == 10
    at += 1
    rgb = data[at:at + 192]
    assert len(rgb) == 192
    at += 192
    references.append({"name": name, "source": background + prefix + program, "rgb": list(rgb)})
assert at == len(data)
metadata = result.stderr.decode().splitlines()
version = next(row for row in metadata if row.count(".") == 2 and row.replace(".", "").isdigit())
binary_sha = next(row.split()[0] for row in metadata if row.endswith("/usr/bin/gs"))
destination = "tests/fixtures/eps/stroke-pixel-ghostscript-reference.json" if stroke else "tests/fixtures/eps/pixel-ghostscript-reference.json"
if eps_import:
    destination = "tests/fixtures/eps/import-pixel-ghostscript-reference.json"
if unadjusted:
    destination = "tests/fixtures/eps/stroke-unadjusted-pixel-ghostscript-reference.json"
if color_ramp:
    destination = "tests/fixtures/eps/color-ramp-ghostscript-reference.json"
if bevel_boundaries:
    destination = "tests/fixtures/eps/bevel-boundary-ghostscript-reference.json"
if bevel_outlines:
    destination = "tests/fixtures/eps/bevel-outline-ghostscript-reference.json"
if zero_fill_adjust:
    destination = destination.replace('-ghostscript-reference.json', '-zero-adjust-ghostscript-reference.json')
Path(destination).write_text(json.dumps({
    "license": "CC0-1.0", "ghostscript_version": version, "binary_sha256": binary_sha,
    "source_sha256": hashlib.sha256(source).hexdigest(), "width": 8, "height": 8,
    "device": "ppmraw, 72dpi, GraphicsAlphaBits=1", "cases": references,
    "stroke_adjust": False if unadjusted else "device default",
    "fill_adjust": [0, 0] if zero_fill_adjust else "device default",
}, indent=2) + "\n")
print(f"Generated {len(references)} independent EPS RGB pixel fixtures.")
