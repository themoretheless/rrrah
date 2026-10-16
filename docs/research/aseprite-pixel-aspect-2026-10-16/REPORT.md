# Aseprite pixel aspect

Removed the prior refusal for non-square ASE pixels. Header bytes34/35 now propagate width/height ratio to DecodedRaster; zero in either field or equal dimensions retain square-pixel default metadata. No pixel resampling or buffer expansion is introduced.

Primary contract: https://github.com/aseprite/aseprite/blob/main/docs/ase-file-specs.md (Header: pixel width and pixel height, either zero means1:1). Tag repeat semantics and common animation timing remain separate open work; this change does not invent a global tag policy.

Validation: all65536 header byte pairs agree with an independent f64 ratio reference rounded to f32. Nine aspect cases across three selected frames preserve exact source pixels and native delays. Nine real temporary sources pass common viewer color preparation, RAM count1 eviction, swap restore with decoder fallback disabled, byte-exact payload/aspect equality and final root memory0. Layered library51 passed/1ignored; viewer276 passed/73ignored. Initial run failed an obsolete non-square refusal assertion, removed because this is now supported behavior; failed log retained.

No new actual GPU/physical display qualification is claimed for these mutated sources. Existing renderer pixel-aspect support is reused; original producer non-square sprites and tag-directed playback remain unqualified. Catalogue counts unchanged.

## Actual Metal follow-up

Added explicitly enabled actual Metal qualification using the same nine source header variants. Before eviction and after decoder-disabled swap restore, SDR and RGBA16Float readbacks match exactly. Additional assertions require wide and tall pixels to render differently in both targets, and all four zero/equal-dimension square declarations to render identically. M4 Max passed nine cases,36 readbacks,9 swap restores with final root memory0. Full parallel viewer276 passed/74ignored; this hardware test was enabled and passed separately.

This closes GPU transport/geometry behavior for these authored header variants. It does not establish original producer interoperability, physical HDR scan-out, or tag-directed animation playback.
