# RLA active/full window observation — 2026-10-07

Fixture: committed `rla-8-c3-a0-mixed0.rla`, with full bounds changed to
left=-2, right=142, bottom=-1, top=4; active bounds remain 0..139, 0..2.
Pixel records and row offsets are unchanged.

Native contract: full canvas 145×6; active pixels occupy x=2..141 and y=2..4
in top-down normalized coordinates. The outside is transparent black. Absolute
world origin is not retained in DecodedRaster. Active bounds outside the full
window are rejected, rather than clipped. Directory entries count active rows.

OpenImageIO reports active size 140×3 and full size 145×6, with full origin
(-2,1). Its reader computes data y from active height and full y from full
height independently:
https://github.com/AcademySoftwareFoundation/OpenImageIO/blob/main/src/rla.imageio/rlainput.cpp
This observation validates sizes, not native offset-canvas pixel parity.
Independent real-producer coordinate qualification remains pending.

Native test additionally translates all window coordinates near both i16
extremes, requires byte-identical normalized output and released pixel budget,
and rejects a budget one byte short of the full u16 working canvas.
