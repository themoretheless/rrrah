# 100-format qualification target

The user requested correct operation with 100 image formats. This is an implementation target, not a measured global popularity ranking. Extension aliases (JPG/JPEG, TIF/TIFF) do not add entries. Container families with distinct animation, precision or storage contracts have separate qualification rows.

Completion requires actual library decoding and viewer opening, folder discovery, color management, orientation, precision/alpha preservation, bounded malformed-input handling, and licensed reproducible fixtures with pixel/metadata oracles per row. Animations need disposal, frame timing and loop semantics; layered/page formats need explicit selection/compositing; scientific volumes need explicit slice/window semantics. Camera RAW must decode the sensor, never substitute an embedded preview. Generic TIFF must remain separate from CFA DNG routing.

Current raster API decodes the first image by default. ICO/CUR support quality-ranked image selection; TIFF/BigTIFF and DCX support explicit page indices, WAD3 selects texture entries, PVR/DDS/KTX1/KTX2 select mips, APNG/GIF/WebP/Aseprite select composited animation frames, and NRRD/MRC/FITS select scalar sections or planes and native DICOM selects scalar frames; other animation/page/layer behavior is pending. It preserves ICC bytes; `prepare_raster_for_display` applies RGB ICC profiles to linear sRGB, and marks unqualified PNG/TIFF/EXR color interpretation unspecified pending complete metadata handling. The common `decode_image` API distinguishes sensor and raster results, inspects TIFF sensor tags including SubIFDs, and never falls back from failed sensor decoding to a preview. CLI `--inspect` supports raster inputs; The window now routes prepared still rasters to a dedicated GPU renderer; mixed-folder navigation and color-managed raster thumbnails are connected. 94 of 100 rows have an implemented subset; 6 remain Pending for normal viewer implementation. None of these rows is fully qualified yet.

Reference format inventory: https://imagemagick.org/formats/ (used only as a format reference, not as a runtime dependency).

| # | Format | Current implementation | Completion evidence |
|---|---|---|---|
| 1 | JPEG | Raster first-image decoder; qualification pending | Pillow CC0 baseline pixel/alpha oracle passes; standalone profiled CMYK JPEG with Adobe APP14 transform 0 preserves channels through ICC, matches the independent 512-pixel oracle exactly and passes float display preparation with released managed ownership; standalone profiled YCCK APP14 transform 2 also matches its independent 512-pixel oracle exactly; both fixtures and 16 asymmetric cases covering all eight EXIF orientations pass RAM leases, pressure-preserving native/prepared swap restoration and actual Metal frames against independent references (maximum one RGB8 level); broader profiles, subsampling and physical viewer qualification remain pending; full variants/color/viewer pending |
| 2 | PNG | Raster decode, ICC bytes, sRGB/cICP precedence, gAMA+cHRM explicit color, preserved wide-gamut/HDR/range signaling, straight alpha and 16-bit samples | Pillow CC0 baseline pixel/alpha oracle passes; linear BT.2020/P3, encoded Display P3 and explicit full-range BT.2020 PQ/HLG decode policies tested on authored 16-bit controls. Narrow-range 8/16-bit BT.709 RGB display is supported with signed excursions and unchanged alpha; narrow PQ/HLG remains unsupported and explicit HDR decode policy refuses it. Full variants/color/viewer pending; see research/png-cicp-expanded-2026-10-13.json |
| 3 | GIF | Selected composited RGBA8 frames, delay and repeat metadata; decode stops at selection | Twelve independent Pillow presentations for KEEP/BACKGROUND/PREVIOUS; malformed bounds checked; live playback/full variants pending |
| 4 | WebP | Still decoder plus selected linear-light animation composition, delays/plays, ICC source color | Sixteen libwebp raw-frame/linear presentation contracts, lossy ALPH and lossless; full variants/live playback pending |
| 5 | AVIF | dav1d/image first image, ICC retained; exact pixel-aligned primary clean-aperture crop before transforms | RGB/alpha libavif oracles; ICC and NCLX sRGB display checked; crop rational/order/refusal tests, 24 crop metadata cases against libavif, and eight libavif-exported top-left crops with seven orientations pass native pixel and RAM/swap/Metal qualification; explicit PQ/HLG BT.2020 viewer policies and neutral/colored SVT 10-bit producer decode/swap/Metal float readback pass; independent 12-bit/alpha HDR producers, fractional crop, sequence and physical HDR output pending |
| 6 | HEIF / HEIC | libheif HEVC primary image; ICC, NCLX sRGB/linear and explicit BT.2020 PQ/HLG policies, straight alpha, item transforms and native precision | Four authored lossless pixel oracles and adjacent 12-bit values; six gray/color/straight-alpha 12-bit HEVC PQ/HLG controls preserve CICP and all authored codes, independently reproduced by FFmpeg; primary and mixed-color linear conversion matches independent Decimal/chromaticity vectors, reach linear display with explicit policy, and preserve float bits and Metal SDR/HDR frames through pressure/swap; one external Canon DPP 24 MP PQ photograph passes bounded FFmpeg RGB agreement, managed working admission, swap/Metal and native HDR window; direct-camera HDR producers, premultiplied alpha, other codecs, sequences, physical HDR and full qualification pending |
| 7 | JPEG-XL | jxl-oxide first frame, ICC and native sample precision | libjxl container/codestream alpha oracles; animation/HDR/CMYK/full qualification pending |
| 8 | TIFF | Selected top-level TIFF/BigTIFF pages; qualification pending | Pillow CC0 baseline pixel/alpha oracle passes; full variants/color/viewer pending |
| 9 | BMP | Raster first-image decoder; qualification pending | Pillow CC0 baseline pixel/alpha oracle passes; full variants/color/viewer pending |
| 10 | ICO | Quality-ranked selected PNG/DIB image via ICO decoder; qualification pending | Pillow CC0 baseline pixel/alpha oracle passes; full variants/color/viewer pending |
| 11 | CUR | Selected PNG/DIB images through ICO decoder; matched hotspot and bounds checked | Two independent ICO pixel oracles; hotspot preserved through display; full cursor qualification pending |
| 12 | SVG | resvg shapes/text/embedded sRGB PNG, intrinsic viewport, straight RGBA | CairoSVG shapes and embedded-PNG alpha fixtures; other resources/animation/full qualification pending |
| 13 | EMF | Bounded 88/100/108-byte fixed header with optional UTF16 description rectangle/ellipse and 16/32-bit polygon/polyline/Bezier and continuation subset through SVG; fixed/anisotropic mapping with saved origins/extents, fill modes, MOVETOEX/LINETO current point, saved contexts, stock/custom solid/null RGB brushes/pens and bounded object table with deletion/index reuse | Eight authored opaque fills agree exactly with separate Java2D renderer and pass native/display, RAW-suffix routing, RAM leases, pressure-preserving streamed swap and Metal. Thirty mathematical fixtures pass RAM/swap/Metal; independent EMF-to-SVG/Cairo comparison is 25/30 exact with metric/reflected and continuation mismatches recorded; Bezier cases are collinear; the separate Java2D winding mismatch remains recorded. Independent producers, non-null stroke rasterization, transforms, clipping, fonts/bitmaps, extended headers/color and full qualification pending |
| 14 | APNG | 1/2/4/8/16-bit source frames, selected linear-light float32 composition, timing/repeat metadata | Authored composition and 16-bit precision oracles; Metal SDR/HDR readback; live playback/full color and variant qualification pending |
| 15 | MNG | Native MNG-VLC plus admitted LC global palette/transparency and intrapixel subset; opaque 8/16-bit and linear-light alpha composition, explicit selection/ticks | Authored opaque and 28 alpha presentations preserve native/display samples through RAM/swap and actual Metal; embedded PNG samples independently agree with FFmpeg. Independent MNG compositor, wider control/loops/object placement and physical viewer qualification pending |
| 16 | JNG | Standalone 8-bit JPEG RGB/gray with PNG 1/2/4/8/16-bit or JPEG 8-bit straight alpha, outer color declarations | Eight Pillow JPEG/alpha sample contracts; native adjacent alpha16 preserved; wider JPEG/MNG/live viewer/full qualification pending |
| 17 | JPEG-2000 | OpenJPEG JP2/J2K, unsigned full-resolution RGB/gray/straight alpha, 8/16-bit, RGB ICC | OpenJPEG/FFmpeg pixel fixtures; palette/subsampling/signed/CMYK/full qualification pending |
| 18 | JPEG-LS | Static CharLS gray/RGB, 2–16 bits, planar/line/sample layouts, near-lossless, RGB ICC | 29 native-sample contracts: 13 FFmpeg readbacks, 16 authored lossless expectations; wider color/preset/SPIFF/interop/live viewer qualification pending |
| 19 | JPEG-XR | Static JXRLib gray/RGB 8/16-bit, HALF/FLOAT, separate alpha, ICC, all eight container orientations | 21 typed contracts; independent Rust non-alpha samples plus authored orientation; wider pixel layouts/interleaved alpha/color/interop/live viewer qualification pending |
| 20 | QOI | Raster decoder preserves sRGB/linear transfer flag and alpha | Authored RGBA chunks preserve all source bytes, both transfer flags and alpha through native/prepared swap; physical Metal readback matches constructed-raster display, with pressure/cancel/retry and final credit release. Eight pinned official C-reference fixtures compare every native byte across RGB/RGBA headers, both transfer flags and all six opcode families; regeneration is exact. Native Rust QOI decoding now checks command framing and expands pixels in one pass, reserving output before allocation. Header/end-marker checks and the necessary compressed-length bound precede allocation; impossible bodies reserve no output, while minimal valid run boundaries remain accepted. RGB/RGBA header permutations preserve every encoded alpha byte against the pinned C reference; truncated commands, run overflow and trailing data reject during decoding and release partial-output credit. Cancellation remains checked at every command and releases reservations; independent-reference and Metal swap controls verify the output. Photographic producer corpus, exhaustive malformed variants and physical window qualification pending |
| 21 | TGA | Raster first-image decoder; qualification pending | Pillow CC0 baseline pixel/alpha oracle passes; full variants/color/viewer pending |
| 22 | PNM | Raster first-image decoder; standard P1–P6 BT.709 preparation | ASCII/binary PBM/PGM/PPM and full-range 8/16-bit color checks pass; 65,536-value nominal reference and independently modeled Netpbm toe difference recorded; Metal frame and payload color-tag roundtrip pass. PAM, nonstandard linear/sRGB variations and broader qualification remain separate. |
| 23 | PAM | First image; RGB/gray/black-white with straight alpha and native maxval scaling | Five authored tuple/alpha oracles, including adjacent 16-bit values; transfer/page/viewer/full qualification pending |
| 24 | PFM | Native float RGB/grayscale, bottom-up, both endians; scale retained, output admitted before allocation and wide-row cancellation bounded | Authored binary32 oracle, explicit linear-sRGB CLI, preload/swap/thumbnail, Metal SDR/HDR and native-window checks pass; broader corpus and alternate dialects pending |
| 25 | Farbfeld | Native directly decoded RGBA16, output admitted before allocation; explicit assumed-sRGB import, straight alpha | Adjacent 16-bit sample oracle and linear-display alpha checks; native and prepared disk-swap payloads plus actual Metal frames remain exact with pressure/cancel/root-release checks; compressed wrappers, broad corpus and full qualification pending |
| 26 | OpenEXR | First selected RGB(A) surface; HALF/FLOAT retained as f32 and associated alpha converted to straight | OpenEXR NONE/RLE/ZIP/PIZ pixel oracles; single-part pixelAspectRatio retained through display preparation without resampling; explicit invertible linear chromaticities/white and known Rec.709, BT.2020, P3, ACES AP1/AP0 colorInteropID supported; native wide-gamut preparation and swap pressure/cancellation recovery match analytical SDR/HDR on Metal; zero-alpha emission explicitly refused; other interop IDs, different adopted neutral, layers/deep/tiled/full qualification pending |
| 27 | Radiance-HDR | RGBE float samples; explicit valid invertible PRIMARIES/white prepares native linear sRGB with Bradford adaptation; absent primaries remain unspecified | Authored old-style and scanline RLE pixel oracles; bounded explicit-primary parser rejects malformed/duplicate declarations, declared Rec.709 RLE HDR preserves all native samples and shares prepared storage; declared BT.2020/D65 native matrix, managed retry and swap-pressure/cancellation recovery preserve analytical SDR/HDR frames on Metal; actual Metal resident-exposure completed-frame timing passes; arbitrary explicit primaries/non-D65 white verified on three authored RGBW sources including AP1/AP0 through managed preparation and swap retry with exact Metal SDR/HDR; exposure calibration, orientation, wider corpus and physical HDR output pending |
| 28 | PSD | Native saved RGB/gray composition, raw/RLE/ZIP/prediction, 8/16/32-bit; ICC resources | FFmpeg raw/RLE and psd-tools merged-alpha fixtures; layers/color modes/full qualification pending |
| 29 | PSB | Native saved composition with 64-bit section lengths and 32-bit RLE row lengths | psd-tools raw/RLE pixel oracles; full large-document qualification pending |
| 30 | XCF | Partial: legacy v0-v3 8-bit normal-layer flatten routes through decode_image/decode_raster; ICC preserved, managed source/canvas/pixels/profile, cancellation, gallery detection and SDR Metal/swap fixture transport verified. Native selected-layer/mask and group metadata APIs remain available. | 12 bounded legacy normal-layer compositions exactly match pinned independent parsing and Pillow alpha composition, including gray, opaque indexed, endpoint masks and one 128/255 opacity case; dimensions/managed release verified. Modern precision, groups/effects/non-normal modes, general fractional masks/opacity, indexed-alpha, v0 indexed interpretation and full color/viewer qualification remain. Untagged inputs require explicit sRGB interpretation. No physical presentation proof; see research/xcf-current-full-flatten-2026-10-06.json and research/xcf-raster-route-metal-2026-10-06.json. |
| 31 | KRA | Saved full-canvas PNG composition through common color pipeline | Python ZIP/Pillow pixels, ICC display and MIME routing checked; native tiles/layers/animation/full qualification pending |
| 32 | ORA | Stored composition PNG through common color pipeline | Python ZIP/Pillow RGBA and ICC fixtures; layers/recomposition/full qualification pending |
| 33 | Aseprite | Selected native RGBA/gray-alpha/indexed frames, raw/zlib/linked cels, normal layer/group composition, palettes, color profiles and timing/tags | 63 selected-frame contracts (39 independent asefile, 24 authored); wider blends/tilemaps/precise bounds/full interop/live playback/viewer qualification pending |
| 34 | DDS | Native BC1/BC2/BC3 and masked RGB16/24/32 selected 2D mip | Thirteen baseline Pillow/FFmpeg fixtures and 36 selected-mip Pillow oracles; other texture layouts/full qualification pending |
| 35 | KTX | Native KTX1 selected 2D mip: RGB/RGBA 8/16/float32 and BC1/2/3, orientation/endian handling | Authored PNG/native sample and independent DDS block oracles, including 60 selected mips; array/cube/volume/other storage/full qualification pending |
| 36 | KTX2 | Native ordinary RGB/RGBA/BGR/BGRA 8-bit, RGB/RGBA16 and float32; plain/DEFLATE, DFD and orientation | Twelve authored pixel fixtures, four precision files and 96 selected-mip native sample oracles; BasisLZ/UASTC/Zstd/compressed textures/array/cube/volume/viewer/full qualification pending |
| 37 | Basis | ETC1S/UASTC LDR transcoding to managed RGBA8 with selected image/mip, file sRGB flag, CRC and bounded output admission | Eight authored files and 32 selected mips/images (white, 7x5 colored-alpha blocks and two-image red/green mip chains) pass RAM leases, 64 native/prepared pressure-preserving streamed swap roundtrips and actual Metal readback. White reference is exact; colored-alpha channels agree within fixed error 8/255, and swap samples are bit-exact. Four pinned upstream ETC1S/UASTC producer files additionally pass all 13 mip/image selections and 26 native/prepared RAM/swap/Metal preservation roundtrips with SHA256-verified inputs; four newer codecs explicitly refuse. Independent pixel/color oracle, full producer coverage, complex spatial textures/alpha, array/cube/Y-flip, KTX2 Basis and direct compressed GPU upload qualification pending |
| 38 | PVR | v3 ordinary 8/16/float32 RGB channels, PVRTC1 2/4 bpp, mip selection, orientation metadata | Authored native samples and PowerVR SDK raw-PVRTC oracle; other codecs/legacy/cube/array/packed/viewer pending |
| 39 | ASTC | Arm reference decoder, all 14 2D footprints; RGBA8 LDR / RGBA32F HDR, validated block classification | 14 reference pixel oracles; compressed and constant HDR precision checked; explicit linear-sRGB HDR preload/swap and actual Metal SDR/HDR preservation pass; 3D/linear-LDR/producer/full qualification pending |
| 40 | PKM | Bounded ETC1/ETC2 RGB/RGBA8/RGBA1 color textures via Rust block decoder | Android ETC1 oracle, cropping and baseline ETC2 alpha cases; full ETC2/EAC qualification pending |
| 41 | DNG | Bounded native mosaic decoder; full qualification pending | 1 pinned current corpus case(s) pass full sensor hash, geometry/CFA, WB/matrix and declared metadata contracts against independent LibRaw references under 256 MiB managed admission; broader variants and physical display remain open. See research/current-managed-raw-corpus-2026-10-06.json. |
| 42 | CR3 | Bounded native mosaic decoder; full qualification pending | 2 pinned current corpus case(s) pass full sensor hash, geometry/CFA, WB/matrix and declared metadata contracts against independent LibRaw references under 256 MiB managed admission; broader variants and physical display remain open. See research/current-managed-raw-corpus-2026-10-06.json. |
| 43 | CR2 | Bounded native mosaic decoder; full qualification pending | 1 pinned current corpus case(s) pass full sensor hash, geometry/CFA, WB/matrix and declared metadata contracts against independent LibRaw references under 256 MiB managed admission; broader variants and physical display remain open. See research/current-managed-raw-corpus-2026-10-06.json. |
| 44 | CRW | Native Canon EOS 10D CIFF/table-zero subset | All 6,518,336 sensor samples match independent LibRaw 0.22.2 through the public RAW router. Strict WB/profile/crop/orientation, managed admission, real mixed-gallery prefetch/disk restore, lease TTL/size/count, swap pressure retry and Metal frame preservation verified. Other cameras/tables/storage, live navigation and photographic/physical-display qualification remain open. See `research/crw-public-sensor-oracle-2026-10-06.json` and `research/crw-router-prefetch-metal-2026-10-06.json`. |
| 45 | NEF | Bounded native mosaic decoder; full qualification pending | 1 pinned current corpus case(s) pass full sensor hash, geometry/CFA, WB/matrix and declared metadata contracts against independent LibRaw references under 256 MiB managed admission; broader variants and physical display remain open. See research/current-managed-raw-corpus-2026-10-06.json. |
| 46 | NRW | Native subset: COOLPIX P7700/P7800 mode 7 | Sensor/color oracle, swap and Metal readback; other cameras pending |
| 47 | ARW | Bounded native mosaic decoder; full qualification pending | 2 pinned current corpus case(s) pass full sensor hash, geometry/CFA, WB/matrix and declared metadata contracts against independent LibRaw references under 256 MiB managed admission; broader variants and physical display remain open. See research/current-managed-raw-corpus-2026-10-06.json. |
| 48 | SRF | Native subset: Sony DSC-F828 encrypted RGBE sensor | CC0 object 1351: all sensor samples match LibRaw; explicit four-plane WB/profile, CPU thumbnails, RAM/disk/swap and independent Metal frame readback qualified. Other models, tiled/cropped RGBE variants and physical color/display qualification remain. |
| 49 | SR2 | Native DSC-R1 14-bit uncompressed sensor and strict color metadata | Object 3221: all 10,390,272 samples, GRBG black levels from encrypted 0x7300, WB, matrix, white level and 3925×2608 crop match LibRaw. Persistent cache and swap preserve full pixels/metadata; actual Metal readback matches the independent sensor/color reference. Other cameras/storage profiles and full photographic color qualification pending. See `tests/fixtures/sr2-qualification.json`. |
| 50 | ORF | Bounded native mosaic decoder; full qualification pending | 1 pinned current corpus case(s) pass full sensor hash, geometry/CFA, WB/matrix and declared metadata contracts against independent LibRaw references under 256 MiB managed admission; broader variants and physical display remain open. See research/current-managed-raw-corpus-2026-10-06.json. |
| 51 | RW2 | Bounded native mosaic decoder; full qualification pending | 1 pinned current corpus case(s) pass full sensor hash, geometry/CFA, WB/matrix and declared metadata contracts against independent LibRaw references under 256 MiB managed admission; broader variants and physical display remain open. See research/current-managed-raw-corpus-2026-10-06.json. |
| 52 | RAW-Panasonic | Native subset: FZ50 mode 34828 left-aligned and FZ8 mode 34316/format 2 packed; independent sensor oracles; other legacy profiles rejected | Persistent cache, pressure/retry swap and Metal readback for both cameras; broader cameras pending |
| 53 | RAF | Bounded native mosaic decoder; full qualification pending | 1 pinned current corpus case(s) pass full sensor hash, geometry/CFA, WB/matrix and declared metadata contracts against independent LibRaw references under 256 MiB managed admission; broader variants and physical display remain open. See research/current-managed-raw-corpus-2026-10-06.json. |
| 54 | PEF | Bounded native mosaic decoder; full qualification pending | 1 pinned current corpus case(s) pass full sensor hash, geometry/CFA, WB/matrix and declared metadata contracts against independent LibRaw references under 256 MiB managed admission; broader variants and physical display remain open. See research/current-managed-raw-corpus-2026-10-06.json. |
| 55 | PTX | Pending | PEF-compatible extension routing tested; independent camera-produced PTX source still missing |
| 56 | SRW | Native Samsung EX1 little-endian sensor words and strict keyed WB in TIFF | Object 1204: all 10,252,640 samples and color metadata match LibRaw; persistent cache, swap pressure retry and whole 128×96 Metal frame verified. Other Samsung layouts and full qualification pending. |
| 57 | RWL | Native RW2-family subset; D-LUX Typ 109 oracle | Metal/swap readback; physical display and broader cameras pending |
| 58 | DCR | Native DCS760C compression-65000 sensor and strict native WB/profile | All 6,128,640 samples and color metadata match LibRaw under 128 MiB managed budget; persistent cache, swap pressure retry, Metal frame and five malformed sources qualified; broader Kodak variants and full qualification pending. |
| 59 | KDC | Native Kodak P880 packed12 subset; other cameras pending | CC0 object 2339: all 8,049,120 sensor values and color metadata match LibRaw; five native WB selections independently checked. Persistent cache/swap exact roundtrip and whole-frame Metal readback verified; eight malformed cases rejected without preview fallback or retained memory. Full photographic color and physical display pending; see `tests/fixtures/kdc-qualification.json`. |
| 60 | MRW | Native Dynax 7D and DiMAGE A2 packed 12-bit mode 0x59, managed sensor decode and strict camera color/levels | CC0 objects 1826 and 4419: full sensor samples, WB, calibrated matrix, CFA, crop, black/white levels independently qualified; native RAW and mixed-image routing connected. Persistent cache and swap preserve all pixels/metadata; actual Metal readback matches independent samples/color. Other cameras/storage modes, visible window presentation and full photographic color qualification pending; see `tests/fixtures/mrw-qualification.json`. |
| 61 | MOS | Native Leaf Aptus 22 MM TIFF, single compression-99 lossless JPEG tile; bounded PKTS WB, exact camera profile, full 21,418,752-sample LibRaw equality and managed allocation; other cameras/layouts unqualified | Required RAW corpus, RAM lease/limits, disk/swap pressure retry and full Metal frame equality pass; 6 malformed cases and low-budget refusal release allocations |
| 62 | MEF | Pending: explicit native Mamiya ZD sensor import API | Known MEF TIFF classifies directly as sensor. Public native decode explicitly refuses the unqualified MEF color path through `NativeFormat::UnqualifiedMef`, rather than routing through DNG SubIFDs. No preview fallback or retained managed credit. See research/mef-explicit-color-refusal-2026-10-11.json. Public `decode_mef_zd_sensor` returns managed sensor codes only; full sensor comparison remains independent of viewer qualification. Native WB remains unresolved. Public TIFF structure and color support pending; see tests/fixtures/mef-investigation.json |
| 63 | ERF | Native Epson R-D1 subset; other profiles pending | CC0 object 2680: all 6,152,960 sensor samples and strict native color metadata qualified against LibRaw; persistent cache/swap exact roundtrip, pressure retry and whole-frame Metal readback verified. Five malformed variants rejected without preview fallback or retained managed memory. Physical display and universal ERF coverage pending; see `tests/fixtures/erf-qualification.json`. |
| 64 | IIQ | Native Phase One P20+-H format-3 subset; other cameras/storage pending | CC0 object 4366: all 17,065,152 samples after native black, pixel, gain-grid and isolated-column correction match independent LibRaw stages. Native WB/profile/crop, RAM leases, persistent cache, bounded swap pressure retry and whole-frame Metal readback verified. Seven malformed cases rejected with budget zero. Other variants, physical display and full photographic color pending; see `tests/fixtures/iiq-investigation.json`. |
| 65 | 3FR | Native uncompressed Hasselblad X1D full sensor with file-origin WB/matrix/levels/crop | Object 2058: all 52,852,736 samples match LibRaw. Embedded matrix precision and stored default crop retained; persistent cache, swap pressure retry and consistent 128×96 Metal frames verified. Stored matrix/WB/levels/crop independently match tifffile 2026.9.20. Photographic color accuracy and broader layouts pending. |
| 66 | FFF | Native CFV-50 predictor-8 sensor, native WB and exact FFF profile | Object 1637: all 51,679,680 samples and color match LibRaw; persistent cache, swap RAM-pressure retry and whole Metal frame verified. Broader camera/codec variants and full qualification pending. |
| 67 | X3F | Native Sigma SD10/SD14 legacy Huffman/CAMF subset, strict selected WB_DESC, linear RGBA16 and raster viewer routing | Pinned SD10 Auto 2267×1513 and SD14 Sunlight 2639×1757 public output match independent standalone converter channel-for-channel. SD14 explicit Auto is also exact. RAM/swap exact restoration, preload windows, generation cancellation and completed Metal rendering verified. Linear fixture full-frame Metal readback deviation <=1 code value. Other cameras/encodings and full-image qualification of additional WB modes remain pending, as do physical display/HDR; see `research/x3f-sd14-public-sunlight-2026-10-06.json` and `research/x3f-reproducible-full-output-2026-10-06.json` |
| 68 | RWZ | Pending: Rawzor compressed-file wrapper | Requires native reconstruction of original image payload, then routing to its actual decoder. ExifTool Rawzor.pm documents the 46-byte little-endian header and bzip2 metadata sections; it reconstructs metadata with image-data gaps zero-filled, not original sensor pixels. Header/metadata inspection alone must not count as RWZ image decoding. No qualified pixel codec or reproducible producer fixture yet; see research/rwz-container-boundary-2026-10-08.json. |
| 69 | BAY | Pending: native explicit-layout sensor unpack for Casio QV-2000UX, QV-3000EX and QV-5700 | `BaySensorLayout::image_samples_per_row` separates QV-5700’s documented 2576 image samples from full-row unpack storage containing trailer positions. Camera-file qualification, CFA/color metadata and viewer routing pending |
| 70 | CAP | Pending: legacy Phase One / Capture One sensor format | Current native IIQ path is explicitly P20+ format3 with model-specific calibration; it does not establish CAP compatibility. Fresh raw.pixls.us snapshot contains 2016 rows, 43 Phase One rows and no CAP/CAPTURE download suffix. Original container/sensor/color oracle and native viewer route remain required; see research/cap-source-boundary-2026-10-12.json. |
| 71 | EIP | Original RAW sensor subset through common viewer route | Bounded classic ZIP, native RAW extraction, shared budget, package-file cache fingerprint and inner decoder recipe. Authored EOS R8 packages pass actual forward/backward app prefetch, one-entry disk cache, RAM TTL/swap and Metal frame preservation against original CR3. Capture One-produced packages and settings/ICC/LCC appearance remain unqualified. See research/eip-common-route-2026-10-07.json, research/eip-app-prefetch-2026-10-07.json and research/eip-ttl-swap-metal-2026-10-07.json. |
| 72 | DCS | Native Kodak DCS520C lossless-JPEG TIFF subset, strict WB/profile/response curve | CC0 object 2573: original TIFF routes to backend 22 revision 1; all 2,013,760 sensor samples match LibRaw; managed ownership, RAM lease, disk/swap restore and Metal frame readback pass. Exact observed extra JPEG tail accepted only in this camera path; other cameras/tails, physical navigation/color/display and full qualification remain. |
| 73 | EPS | Native bounded Gray/RGB vector subset through common raster/viewer route; content-based framing and neighbour discovery | Managed source/compiler/VM/path/fill/stroke pipeline; numeric operators, rectangle forms and signed-radius arc/arcn cubic paths with bounded multi-turn sweeps; native clip/eoclip intersections and initclip retain paths and restore with gsave/grestore; numeric rectclip clears the active path with bounded rollback on failure; RAM size/count, visible pin/TTL, swap pressure/cancellation/restore and both neighbour directions tested. Actual Metal offscreen pixels match analytic references after swap restore. Selected arc pixels are qualified with explicit one-sample coverage and zero oracle fill adjustment; the ellipse boundary and default-coverage parity remain open. Strict independent grayscale quantization and diagonal stroke pixel gates remain unresolved; arrays/dictionaries, clippath and array/string rectangle forms, fonts/images, stroke adjustment and full EPS variants remain unsupported. CPU benchmark covers authored 512x512 cases only. See research/eps-updated-app-metal-2026-10-07.json and research/eps-alpha-fast-path-2026-10-07.json. |
| 74 | AI | Pending: `.ai` gallery candidate; PDF-compatible content uses the PDF renderer, including selected pages. PostScript AI remains unsupported. | Pinned PDF-compatible AI has RAM/page-key and swap/Metal preservation evidence; the 2026-10-08 strict Poppler comparison fails on two of three pages. Production fidelity and PostScript AI remain pending (research/ai-current-poppler-2026-10-08.json). Test-only native axial/radial, Type4 triangle and admitted Type7 tensor sources preserve original device components. The authored 512-pixel Type7 CMYK stream is exact. Dynamic capacity admission captures all five real Tensor sources (298,173 / 180,269 / 180,269 / 24,811 / 62,901 triangles) within 256 MiB without loosening error bounds. Shared curved clips and bounded original Separation/DeviceN tint functions now replay all 2,755 VectorApple events and both one.ai pages, with final admitted bytes zero. All 512 authored nonlinear DeviceN CMYK pixels match the independent float oracle (105 interpreter / 126 compositor tests). Strict native RGB comparison still fails on VectorApple (33,072 pixels) and one.ai page0 (2,494 pixels); page1 is exact. Completion of replay is not fidelity qualification. See research/pdf-native-devicen-calculator-2026-10-10.json and research/ai-native-devicen-poppler-2026-10-10.json. This is not production AI qualification. See research/pdf-adaptive-dynamic-admission-2026-10-10.json. |
| 75 | STI | Native indexed8/ETRLE, RGB16/24/32 masks and bounded zlib; explicit subimage selection, managed output | Ten authored images and twenty external JA2 images agree with sample references; external parser plus original blitter resolves literal-zero coverage; native/prepared RAM/swap/Metal checks pass. Independent RGB/zlib producers, placement/application semantics, color and full qualification pending |
| 76 | PDF | Hayro 0.7.1 selected page raster at one pixel per point, bounded geometry/page count, source/pixmap/final-buffer reservations and straight RGBA8 conversion | Two authored opaque 10x10 pages match Poppler and ordinary native routing exactly; managed ownership and insufficient budget/index refusals pass. Both selected opaque pages and a constant half-alpha page pass RAM leases, six native/prepared pressure-preserving streamed swap restores and actual Metal frame readback against mathematical colors. Fixed-profile DeviceCMYK float fills use native hybrid ICC stages: 4721 independent LittleCMS vectors stay within one RGB8 level, an opaque CMYK PDF matches exactly and passes RAM/swap/Metal. Parser/render scratch, interruptible rendering, broader transparency/fonts/color/profiles, independent producer breadth and physical presentation qualification pending |
| 77 | GPR | Native managed VC-5 sensor decode and normal viewer quality development; full qualification pending | Native managed VC-5: six official HERO5/6/7/9 and Fusion sources, 77,875,968 sensor values exact against complete GoPro SDK. HERO gain maps applied in default quality development; RAM limits, previous/next preload, cancellation, sensor/developed-float swap and Metal tested on HERO9. Native Fusion rectilinear development enabled; synthetic full warp-image comparison against SDK passes; dual-illuminant matrices and 24 color fields for all six cameras independently verified on Metal; full-image color/demosaic quality and broader variants remain unqualified (research/gpr-six-camera-matrices-2026-10-07.json); see research/gpr-full-sdk-sensor-oracle-2026-10-07.json and research/gpr-developed-float-swap-metal-2026-10-07.json |
| 78 | XBM | Native X11 byte / X10 short monochrome | Pillow-encoded / FFmpeg-decoded X11 fixture; X10 synthetic checks; full syntax/hotspot qualification pending |
| 79 | XPM | Native XPM2/XPM3 palette and transparency | FFmpeg multi-character XPM3 oracle; XPM2 synthetic; full named colors/extensions/hotspots pending |
| 80 | PCX | Native RGB/indexed/planar RLE decoder; explicit assumed-sRGB import | Synthetic and Pillow CC0 pixel oracles; linear display preparation checked; full qualification pending |
| 81 | DCX | Native directory and selected PCX page via image_index | Two-page Pillow pixel oracle; malformed offsets and out-of-range selection tested; CLI and bracket-key page routing implemented; live window navigation pending |
| 82 | SGI | Native 8/16-bit normal-color, verbatim/RLE | RGB/RGBA Pillow pixel fixtures; full variant qualification pending |
| 83 | SUN-Raster | Native 1/8/24/32-bit, palette, RGB/BGR and RLE | Eight independent Pillow/FFmpeg fixtures; other variants pending |
| 84 | IFF-ILBM | Native 1–8 planes/palette, 24-bit RGB, mask/index transparency, EHB/HAM6, raw/ByteRun1 | Ten independent RGB fixtures plus authored HAM mask alpha; HAM8/animation/aspect/full qualification pending |
| 85 | IFF-PBM | Native 8-bit indexed bitmap, raw/ByteRun1, transparent index | Four FFmpeg pixel/alpha fixtures; aspect and full qualification pending |
| 86 | PICT | Native v2 rectangles and packed RGB555/planar RGB8 DirectBitsRect; raw xRGB/RGB555 and drop-pad RGB; rectangular clipping, source cropping and extended resolution | Authored CC0 RGBA fixtures, four packed external pixel oracles and three exact macOS xRGB/drop-pad cases; RAM/swap/Metal transport and explicit viewer sRGB policy checked; unpacked RGB555 independent disagreement, broader commands, producer color and corpus licensing qualification pending |
| 87 | MacPaint | Native 576x720 data fork and MacBinary I/II/III, versions 0/2, row-wise PackBits, opaque RGBA | Ten pixel oracles plus CRC/bounds/routing checks; AppleDouble/native-app/viewer/full qualification pending |
| 88 | WMF | Native placeable geometry subset through SVG; solid/null pens/brushes, polygons/polylines, rectangle/ellipse/roundrect, window/viewport state, save/restore and free-object deletion | Six authored fills agree exactly with libwmf and pass RAM/swap/Metal; malformed/index/budget tests. External arrow01 decodes but differs in 1,181 white-matted pixels; curved corners lack a suitable independent oracle. Fonts/bitmaps/clipping/other map modes, full variants/color and physical viewer qualification pending |
| 89 | FITS | Primary/IMAGE scalar arrays; all native BITPIX types, BSCALE/BZERO, BLANK, HDU/section selection, explicit window | 22 Astropy native/physical/window plane oracles and malformed/precision checks; compressed/WCS/live viewer pending |
| 90 | DICOM | Native Part 10 Explicit VR LE MONOCHROME1/2 8/16-bit scalar frames, signed/stored-bit extraction, rescale and explicit window | Four pydicom pixel/rescale fixtures; compressed/implicit/big-endian syntax, sequences/LUT/color and clinical presentation/full qualification pending |
| 91 | NRRD | Partial | Attached scalar 2D/3D uint8/uint16/int16/float32, raw/gzip/text; explicit window |
| 92 | MRC | MRC2014 scalar int8/int16/uint16/float16/float32 sections; both endians, explicit window | 20 mrcfile raw-unit plane oracles, exhaustive half conversion, metadata/bounds/routing checks; other modes/live UI/physical-space rendering pending |
| 93 | DPX | Unsigned single-element RGB/RGBA/ABGR/luma, 8/10/12/16-bit, endian and orientation handling | 24 FFmpeg exact sample oracles; authored orientation/padding/ABGR16 and malformed checks; full color/variants/viewer pending |
| 94 | Cineon | V4.5 unsigned pixel-interleaved gray/RGB, 8/10/16-bit, endian/orientation/padding | 12 OpenImageIO sample oracles; malformed header and RAW-suffix routing checks; log/color/variants/viewer pending |
| 95 | RLA | Indexed gray/RGB 8/16-bit byte-plane RLE and explicit-endian float32; explicit matte alpha/color interpretation; contained active/full windows | 14 OpenImageIO sample oracles, reordered rows, mixed precision, composition/bounds checks; offset canvas placement has native tests only; producer coordinate/color/channel and live viewer qualification pending |
| 96 | RPF | Explicit configured common viewer import; native bounded raster with producer pixel aspect | Common request and app RGB/matte/alpha/color flags support foreground and preloads; all interpretation settings separate cache keys; versioned swap preserves aspect and standard Metal upload applies it; actual explicit HDR/alpha RAM/swap and rectangle tests pass; renamed PNG/CR3/unknown files route by header with preserved pixels/aspect; gallery discovery of unknown suffixes, linked-image selection, real-producer corpus and broad qualification pending |
| 97 | PIC-Softimage | Native 8/16-bit RGB/alpha packets, raw/pure/mixed RLE; assumed sRGB | 12 OpenImageIO exact sample fixtures, malformed packets/runs and RAW-suffix routing; full variants/viewer pending |
| 98 | PIX-Alias | Native row RLE, 8-bit gray/24-bit BGR, assumed sRGB | FFmpeg pixel oracles, row bounds and maximum run checked; full qualification pending |
| 99 | WAL | Quake II base mip; explicit RGBA palette API or game-tree PCX palette | Two Pillow palette-override pixel oracles; game-tree routing and malformed mip/palette checks; full variants/viewer pending |
| 100 | WAD | WAD3 indexed mip textures with embedded palette and selected texture index | Two archives/four image oracles via vgio mip parser + authored Pillow palette; bounds/index/routing checks; full WAD variants/viewer pending |

Next work: qualify the connected common decoder/viewer/gallery against live opening, color and metadata contracts; implement the remaining codecs and per-format fixture gates; add animation, page/layer and scientific slice semantics. Update this matrix from executed evidence rather than compiled feature flags.

PFM contract: Debevec/Netpbm bottom-up raster; finite nonzero scale preserved separately as `DecodedRaster::sample_scale`, without altering samples. The indistinguishable Adobe top-down dialect uses explicit `DecodeRequest::pfm_top_down` / viewer `--pfm-top-down`; bottom-up remains default. Authored RGB/grayscale and both-endian regressions pass, with distinct viewer RAM/swap row-policy keys and exact Metal SDR/HDR restore frames; independent Adobe-exported corpus remains pending. Primary references: https://pauldebevec.com/Research/HDR/PFM/ and https://netpbm.sourceforge.net/doc/pfm.html .

Color preparation: `DecodedRaster::to_linear_srgb` converts declared sRGB or linear sRGB to straight-alpha float RGBA, retaining HDR and advisory sample scale. Unknown primaries/ICC, nonfinite samples, and alpha outside 0..1 are explicit errors. PNG gamma/chromaticities and non-sRGB cICP transforms, TIFF/EXR primaries, grayscale/CMYK ICC and viewer integration remain pending.

PNG metadata qualification: sRGB and supported RGB cICP (BT.709 primaries with sRGB or linear transfer) reach color conversion. cICP takes precedence over ICC/sRGB; unsupported cICP stays unspecified rather than falling back. Color chunk CRCs, duplicate declarations and invalid lengths/ranges are checked. Reference: https://www.w3.org/TR/png-3/ .

Independent baseline corpus: `tests/fixtures/raster/manifest.tsv` records 13 fixtures across nine container formats, including progressive JPEG, BMP-backed ICO and indexed PNG. Fixtures and straight RGBA8 oracles are generated by Pillow via `scripts/generate-raster-fixtures.py`; source SHA256 and oracle dimensions verified. `raster_corpus` tests compare every decoded sample through `decode_image_file` (exact lossless, JPEG tolerance two levels) and reject truncated headers. This does not qualify animation, all compression variants, ICC rendering or window display.

RGB ICC preparation: pinned moxcms 0.8.1, relative colorimetric intent, extended-range f64 transform to linear sRGB, checked f32 output. Alpha bypasses color transform and source pixels remain unchanged. Tests cover 16-bit gray/adjacent samples, Display P3 gamut excursions, invalid profiles, and an independent LittleCMS sRGB profile embedded by Pillow (RGB tolerance 1/1024 linear, alpha 1e-7). Normalized finite RGB ICC inputs are required; grayscale/CMYK need retained source channel layouts before transformation.

Raster GPU backend: `RasterRenderer` uploads bounded linear sRGB RGBA32F textures and supports fit/zoom/pan/exposure, bilinear sampling with premultiplication before interpolation, linear alpha compositing and one hardware sRGB display transfer. Metal Apple M4 Max readback verified gray brightness, partial-alpha compositing and transparent-RGB fringe suppression. Single-texture limits are explicit errors. Window routing, mixed-folder discovery/navigation and raster thumbnails are connected. Raster cache, HDR tone mapping, full animation/page selection and larger tiled rasters remain pending.

Window integration qualification: foreground raster decode/color preparation bypasses sensor-mosaic caches, respects generation checks and shared decode admission, and switches renderers while releasing the old GPU texture. Untagged PNG uses an explicitly labeled sRGB fallback; declared unsupported color spaces do not use that fallback. Thumbnail test covers independent ICC PNG and alpha compositing. Interactive visual/navigation verification is pending because macOS was locked during the attempted UI inspection.

SGI normal-color decoding follows the [original SGI specification](https://ftp.zx.net.nz/pub/archive/ftp.sgi.com/graphics/grafica/sgiimage.html). Grayscale, RGB and RGBA are accepted; obsolete colormaps and other channel layouts are rejected. Color is explicitly assumed sRGB. RLE bounds, shared offsets and 16-bit precision have synthetic regression checks.

Sun Raster parsing uses the [Sun rasterfile manual](https://www.cs.cmu.edu/~maxwell/misc/vascHelpPages/sunRasterFormat.html) for layout and [FFmpeg storage constants](https://ffmpeg.org/doxygen/7.0/sunrast_8h_source.html) for byte encoding and RGB/BGR variants. Unknown storage and arbitrary colormaps are explicit errors. Untagged samples use the explicit assumed-sRGB import convention.

32-bit Sun samples use XRGB/XBGR with leading ignored padding, as in the [FFmpeg decoder](https://ffmpeg.org/doxygen/7.0/sunrast_8c_source.html). Three FFmpeg-oracle fixtures exercise RGB/BGR and RLE with varying padding; output alpha is opaque. Pillow instead assumes trailing padding and is not the oracle for these cases.

DDS BC1/BC2/BC3 decoding preserves transparent BC1 pixels and BC2/BC3 alpha. Legacy color uses an explicit assumed-sRGB convention; DX10 typed UNORM uses linear sRGB and SRGB uses sRGB. Typeless data, premultiplied/custom alpha, cube/volume/array textures and other pixel layouts currently return explicit errors. 2D mip selection is implemented through `image_index`, with complete-chain bounds validation. Tightly packed ordinary RGB mip chains and BC1/2/3 use per-level dimensions; padded ordinary mip-chain pitch interpretation remains unsupported. 36 selected-mip native RGBA oracles across nine authored chains pass; independent full-chain loader and live-window qualification remain pending. Header interpretation follows the [Microsoft DDS guide](https://learn.microsoft.com/en-us/windows/win32/direct3ddds/dx-graphics-dds-pguide).

Legacy uncompressed DDS RGB16/24/32 accepts disjoint contiguous masks, normalizes each channel independently, honors explicit pitch, and rejects invalid masks or truncated storage. RGB565, RGB24 and BGRA32 have FFmpeg fixtures; DX10 RGBA8/BGRA8/BGRX8 UNORM and SRGB are decoded with declared transfer functions; wider integer/float formats remain pending.

PCX now uses the explicit assumed-sRGB import convention, allowing display and thumbnails through the common pipeline; this is an import assumption, not a color profile declared by PCX. Version 4 header palettes are accepted according to the [ZSoft reference](https://techheap.packetizer.com/compression/graphics/pcxfmt.html). RGB/indexed common-router fixtures and analytic transfer/alpha checks cover the path.

AVIF decoding uses `image` avif-native with libdav1d >=1.3 (system C library). ICC-profiled RGB inputs use the existing display transform; Primary-item NCLX primaries 1 with transfer 13/8 use sRGB/linear sRGB; other combinations remain color-unspecified. ICC takes precedence when both ICC and NCLX are associated with the primary image. Primary irot/imir transforms apply rotation then mirroring; seven rectangular fixtures cover EXIF-equivalent orientations 2 through 8. Grid images, sequences, EXIF-only orientation and clean-aperture cropping still require qualification. Clean-aperture properties return a typed error.

AVIF transform order follows [libavif transform application](https://github.com/AOMediaCodec/libavif/blob/main/apps/shared/avifutil.h). Fixtures are encoded from EXIF orientations by libavif; Pillow pixel decode plus its independent EXIF transpose supplies the transformed oracle.

XBM is a foreground/background mask. Rrrah imports set bits as opaque black and unset bits as opaque white (matching FFmpeg); Pillow uses opposite display colors. Hotspot metadata and arbitrary C expressions remain pending.

XPM supports hex colors with 1–4 digits per channel, basic X11 color names, c visual entries and None transparency. It uses an explicit assumed-sRGB import convention. Reference: [X.Org XPM format manual](https://www.x.org/docs/XPM/xpm.pdf). Full X11 named-color tables, symbolic overrides and extension metadata remain pending.

XPM hex palettes with 12/16-bit components now produce RGBA16 rather than truncating to bytes. Lower-precision palettes retain RGBA8. Synthetic exact-sample tests check adjacent 16-bit values, 12-to-16 normalization, transparency and distinct linear-display samples. An independent high-depth XPM oracle remains pending.

CUR recognition validates every directory entry reserved byte, nonempty payload, offset beyond the directory and bounded payload range before ICO adaptation. A TGA with a matching four-byte prefix and nonzero obsolete colormap origin has a public-decoder regression check. CUR invalid directories/hotspots return typed errors.

DecodedRaster exposes a validated optional hotspot. CUR chooses the largest image (last entry wins ties), preserves its hotspot and keeps it across linear/ICC display transforms. PNG/DIB cursor integration tests check coordinates (3,5) and reject out-of-bounds hotspots.

SVG uses resvg 0.45.1 and system fonts for text. Source is capped at 64 MiB, output at 512 MiB. External/non-PNG image elements, foreignObject, script and animation are explicit unsupported errors. SVGZ, external references, text font matching, embedded raster color and rendering limits beyond output size require further qualification.

SVG data:image/png resources are validated before rendering, including bounded decoded dimensions/storage, complete PNG decode and supported sRGB interpretation. Embedded ICC/linear/unknown-color PNGs require future preprocessing and return typed errors. External use/href is rejected. A CairoSVG fixture verifies embedded straight-alpha PNG plus vector composition.

OpenRaster validates stored mimetype, unique ZIP entries, bounded stored/deflated streams, stack canvas dimensions and merged PNG dimensions. It follows the [OpenRaster viewer layout](https://www.openraster.org/baseline/file-layout-spec.html); stored composition is read directly. Layer selection/composition and document metadata are pending.

JPEG XL uses a pure Rust decoder with dimension and allocation limits, retains the rendered ICC profile, and applies codestream orientation in the codec. Animation, HDR display, CMYK and full malformed-input qualification remain pending.

JPEG 2000 preflights JP2 boxes, canvas, tiles, component precision and plane/output sizes before invoking bundled OpenJPEG in strict mode. This is not a hard bound on all codec scratch allocations. Bare codestream color remains unspecified; JP2 RGB ICC is retained. Palette mapping, subsampled components, signed or above-16-bit samples, associated alpha, nonstandard color spaces and multiple codestreams are explicitly rejected pending implementation.

DCX selects pages from an ordered, zero-terminated offset directory and bounds each PCX decoder to its page. Explicit nonzero page selection requires a .dcx suffix; default first-page routing uses content magic. Full-table, unordered-directory compatibility and viewer page navigation remain pending. Reference: [Pillow DCX implementation](https://github.com/python-pillow/Pillow/blob/main/src/PIL/DcxImagePlugin.py).

Photoshop decoding reads the stored full-canvas composition, skips bounded layer sections and preserves ICC resources and sample precision. RGB8 merged transparency removes the white matte; grayscale and high precision merged transparency remain explicitly unsupported. Non-RGB/gray modes, layer recomposition and above-65536 dimensions are pending. Format reference: [Adobe Photoshop specification](https://www.adobe.com/devnet-apps/photoshop/fileformatashtml/).

PSD/PSB ZIP streams are decoded incrementally with the exact expected composition size, cancellation checks, and trailing-data rejection. ZIP prediction restores byte/word deltas per row, including float byte-plane permutation. psd-tools supplies ZIP/prediction pixel oracles and independent 16/32-bit precision fixtures.

KRA reads bounded ZIP entries, validates Krita MIME and XML canvas dimensions, accepts maindoc.xml/root, and requires mergedimage.png. Missing saved composition returns an error. Native tile precision, layer reconstruction, animation and KRZ are pending; saved composition does not prove native document fidelity. XML layout follows [Krita saver source](https://github.com/KDE/krita/blob/master/plugins/impex/libkra/kis_kra_saver.cpp) and [KRA tags](https://github.com/KDE/krita/blob/master/plugins/impex/libkra/kis_kra_tags.h).

Alias PIX uses extension fallback after recognized container signatures, preserves opaque RGB/gray bytes, and marks its undocumented transfer as assumed sRGB. Zero-length runs, row-crossing runs, truncation and trailing packets are rejected. Legacy origin fields are ignored, matching [FFmpeg decoder](https://github.com/FFmpeg/FFmpeg/blob/master/libavcodec/aliaspixdec.c). Generic PIX suffix collisions and full variant qualification remain pending.

IFF decoding validates FORM/chunk lengths, singleton BMHD/CMAP/CAMG/BODY, row expansion and output allocation bounds. It returns straight RGBA and explicit assumed-sRGB interpretation. HAM8, animation/delta FORM types, absent-palette interpretation and pixel-aspect metadata remain pending. [FFmpeg IFF decoder](https://github.com/FFmpeg/FFmpeg/blob/master/libavcodec/iff.c) supplies the independent pixel oracle.

HAM6 resets held color to palette entry zero at each row and applies palette/direct R/G/B updates while preserving alpha independently. Three fixtures cover all update commands, row reset, explicit mask and transparent index. FFmpeg RGB is the oracle; HAM mask alpha comes directly from authored source bits because FFmpeg loses that alpha. Dynamic palette/color extensions PCHG/SHAM/CTBL/CLUT/DCOL return explicit unsupported errors. HAM8 remains pending due to different low-bit reconstruction conventions in independent decoders.

PKM validates version, padded/original dimensions, exact payload size and output cap; decodes 4×4 blocks with cancellation and crops only padding. Colors are explicitly assumed sRGB because PKM carries no transfer declaration. Six synthetic fixtures use independent Android ETC1 RGB decoding; the ETC2 cases cover ETC1-compatible RGB, opaque RGBA1 and authored constant-alpha RGBA8. ETC2 T/H/planar modes, variable alpha and punch-through still need independent qualification. EAC R/RG (including signed storage) and obsolete type 2 are explicitly unsupported.

KTX1 validates endian marker, key/value records, dimensions, every declared mip length and padding, then decodes the selected stored 2D mip. RGB/RGBA integer and float samples keep precision; KTXorientation maps left/right and up/down axes. Sized sRGB formats use sRGB transfer, supported non-sRGB formats use the linear RGB texture convention. This does not qualify arbitrary non-color textures or HDR display. Cubes, arrays, volumes, swizzle/associated alpha, half-float and other compressed formats remain pending. Reference: [Khronos KTX1 specification](https://registry.khronos.org/KTX/specs/1.0/ktxspec.v1.html).

HEIF/HEIC uses libheif >=1.17 with an installed HEVC decoder (libde265). HEVC brands are detected independently of extension without capturing generic AVIF brands. The primary image is decoded with strict validation and geometric item transformations; output remains RGBA8 or normalized RGBA16 according to native precision. RGB ICC is retained, NCLX BT.709 primaries with sRGB or linear transfer is recognized, other transfer/primary combinations stay unspecified. Premultiplied alpha is explicitly unsupported. Pixel/plane budgets are checked before and after native decoding; libheif parsing and codec-internal allocations still use the system library's limits. Native decoding cannot be interrupted mid-call. ICC allocation occurs inside the wrapper before the 4 MiB retained-profile limit check. These boundaries remain part of malformed-input qualification.

HEIF fixtures use libheif/x265 lossless RGB encoding, Pillow-authored source/geometry/alpha oracles and secondary heif-convert interoperability checks. Four files cover straight alpha, 90-degree rotation, horizontal mirroring and RGB ICC. A separate adjacent 12-bit sample fixture tests RGBA16 precision. This is not an independent HEVC decoder comparison; grid/crop, other transformations/codecs, HDR profiles, gain maps, image collections, sequences and live viewer opening remain pending.

ASTC `.astc` surfaces use bundled Arm astcenc 5.3.0 through astcenc-sys 0.2.0. The header and exact block count are validated before native decoding, all 14 standard 2D footprints are accepted, and every block is inspected for validity and HDR mode. Arm block-info returns early for constant blocks without setting the HDR flag; validated void-extent FP16 blocks therefore use bit 9 to select HDR. HDR endpoints are never inferred from LDR error colors. HDR samples remain RGBA32Float with unspecified linear RGB primaries; LDR imports use assumed sRGB because the raw container has no transfer/primary metadata. Correct non-color/linear-LDR interpretation still needs an explicit import contract. Three-dimensional surfaces require explicit slice semantics and currently return a typed error. Native output is bounded for worst-case RGBA32F; cancellation is checked during block inspection and around the synchronous decode call, not inside it.

The ASTC corpus has 14 alpha-bearing, cropped 23x17 textures, one per 2D footprint, encoded/decompressed with Arm astcenc 4.6.1 and exact PNG pixel oracles. A separate nonconstant HDR texture compares every float sample against astcenc EXR output extracted by OpenEXR 3.5.2; authored constant FP16 blocks prove values above one and exact alpha survive. These are reference-codec comparisons across versions, not an independent ASTC implementation or live-viewer qualification. Format layout follows [Arm's .astc documentation](https://github.com/ARM-software/astc-encoder/blob/main/Docs/FileFormat.md).

KTX2 ordinary surfaces validate the header, mip index bounds and disjoint ranges, zero padding, Vulkan typeSize, canonical RGBSDA DFD sample layout/ranges and matching transfer. RGB/RGBA UNORM8/16, RGB/RGBA float32 and BGR/BGRA8 are retained without precision conversion. Zero levelCount is accepted for ordinary storage and returns its stored base level. Metadata orientation uses KTX2's two-letter `rd/ru/ld/lu` convention. Associated alpha and swizzle require separate import semantics and are currently rejected. DFD BT.709 primaries map linear/sRGB to the existing display pipeline; other primaries are retained as unspecified. Only the selected DEFLATE level is inflated, with exact declared-size, checksum and trailing-data checks. Every level retains container range and declared-size validation; compressed pixel corruption in unselected levels is reported when selected. The decoded source plus RGBA output is bounded; cancellation is checked during inflation and conversion.

KTX2 fixtures in `scripts/generate-ktx2-fixtures.py` use authored Pillow RGBA values, canonical DFD layouts and Python zlib. Twelve RGBA8 files cover four orientations, plain/DEFLATE, RGB/BGR/BGRA and a lower-mip-first two-level layout; four additional precision files preserve adjacent 16-bit values and HDR floats. These are not independent libktx interoperability fixtures. [Khronos' KTX2 specification](https://github.khronos.org/KTX-Specification/ktxspec.v2.html) and [DFD construction](https://github.com/KhronosGroup/KTX-Software/blob/main/external/dfdutils/createdfd.c) define the container and sample contracts. BasisLZ/UASTC, Zstandard, compressed GPU storage, 1D/array/cube/volume selection and full qualification remain pending.

The native-precision corpus is separate from the RGBA8 corpus: `precision-manifest.tsv` records sixteen successful files with u8/u16/f32 oracles, compared sample-for-sample through the public image API. PAM RGB_ALPHA maxval 15/255, GRAYSCALE_ALPHA/RGB_ALPHA maxval 65535 and BLACKANDWHITE cover scaling, alpha and adjacent 16-bit values. Farbfeld retains big-endian RGBA16 data and uses an explicit assumed-sRGB import following [the format's interoperability recommendation](https://git.suckless.org/farbfeld/file/farbfeld.5.html). Its display conversion preserves straight alpha.

Radiance RGBE fixtures cover raw and modern per-channel scanline RLE; float HDR values remain above one without quantization. Primaries, exposure, color-correction and nonstandard scan order still need separate contracts. OpenEXR 3.5.2 supplies independent HALF/FLOAT NONE/RLE/ZIP/PIZ scanline fixtures and pixel oracles. Stored RGB follows [OpenEXR's associated-alpha convention](https://openexr.com/en/latest/TechnicalIntroduction.html#premultiplied-vs-un-premultiplied-color-channels); the decoder unassociates finite RGB for positive alpha to meet the common straight-alpha API. Zero-alpha/nonzero-RGB emission is a legal EXR case that the current renderer cannot represent correctly; it returns an explicit unsupported-alpha error and has a separate regression fixture. Non-finite/out-of-range alpha and unassociation overflow also return typed errors. Full EXR work requires associated-alpha/emission rendering plus transforms for other colorInteropID/chromaticities and adopted white points, windows, tiled/layer/multipart/deep selection and complete codec coverage. Absence of colorInteropID does not prove linear Rec.709.

Eight EXR composition oracles (`*.exr.over-rgb-f32`) are generated directly from OpenEXR readback of stored associated RGB and alpha over a linear RGB background (0.25, 0.5, 0.75), before any unassociation. The public decoder's straight RGB/alpha must reconstruct that reference composition sample-for-sample for HALF/FLOAT and NONE/RLE/ZIP/PIZ. This verifies composition equivalence at the library boundary, not GPU/window display or zero-alpha emission support.

MacPaint data forks follow [Apple Technical Note PT24](https://leopard-adc.pepas.com/technotes/pt/pt_24.html): 512-byte header, versions 0/2, 720 independently packed 72-byte rows. One bits mean black, zero bits white; output is opaque RGBA8 in canonical sRGB. PackBits literals, repeats and -128 no-op controls are bounded per scanline, truncated payloads and cross-row runs are rejected, and the input is capped at 1 MiB. Trailing no-op controls are accepted; other trailing bytes fail. MacPaint has no strong magic, so extension fallback (`mac/macp/pntg/mpnt`) is used after stronger raster signatures. MacBinary I/II/III are decoded as described below; AppleDouble and live opening remain pending.

Four synthetic CC0 MacPaint fixtures use `scripts/generate-macpaint-fixtures.py`: versions 0/2, non-default pattern tables, literal rows and mixed repeat/literal/no-op rows. Netpbm 11.02.30 macptopbm independently decodes them to PBM, and Pillow supplies exact RGBA8 oracles for all 576x720 pixels. These are data-fork interoperability checks, not native Macintosh application or wrapper qualification.

Netpbm macptopbm 11.02.30 interprets PackBits -128 as repeat-129 instead of no-op. For mixed fixtures, the generator therefore decodes an independently valid equivalent stream without the inserted no-op controls, and uses those pixels as the expected result. No-op identity also has a direct authored unit check. Literal fixture oracles decode the source files directly. This is not direct Netpbm interoperability proof for no-op-bearing streams.

MacBinary PNTG wrappers now extract a bounded MacPaint data fork. I/II/III headers, reserved/version/signature fields, II/III CRC-16/XMODEM, declared fork lengths, secondary-header alignment and zero section padding are validated. Resource data is skipped because MacPaint pixels live in the data fork. Recognizable PNTG wrappers route to raster decoding even with a misleading camera RAW suffix. Six additional Netpbm fixtures cover I/II/III with and without a resource fork; a separate authored test checks secondary-header rounding and zero padding. Get Info comment/secondary-header interoperability beyond this structural check still needs real archival files. Generic `.bin` gallery discovery and AppleDouble are not qualified. CRC and sizes follow the [MacBinary II standard proposal](https://paulbourke.net/dataformats/ascii/).

WAL stores indices, not colors. `decode_wal_with_palette` accepts a caller-supplied straight RGBA palette and color interpretation. The general decoder resolves `pics/colormap.pcx` only when the WAL is under a `textures` directory; this follows Quake II's [palette loading contract](https://github.com/id-Software/Quake-2/blob/master/ref_gl/gl_image.c). The indexed PCX is fully validated, index 255 becomes transparent, and unspecified palette RGB is assumed sRGB. Files without this palette return a typed palette-required error. Four mip offsets and payload bounds are validated; only the base mip is returned. Animation chains, alternate WAL variants, custom palette locations and live viewer qualification remain pending. Folder-thumbnail refresh now invalidates source/palette changes as described below. The CC0 fixtures use authored colors, with exact Pillow WAL-parser oracles and a synthetic PCX for automatic resolution; no game assets or default palette are bundled.

DPX strong SDPX/XPDS signatures route through the common API even with a misleading RAW extension. The bounded decoder preserves 8-bit or normalized 16-bit samples for single-element unsigned uncompressed RGB/RGBA/ABGR/luma; 10-bit RGB uses filled methods A/B and 12-bit uses packed 32-bit words or filled A/B. All eight orientations and declared line/image padding are handled. The storage contract is cross-checked against [FFmpeg's decoder](https://github.com/FFmpeg/FFmpeg/blob/master/libavcodec/dpx.c) and [OpenImageIO's DPX header definitions](https://github.com/AcademySoftwareFoundation/OpenImageIO/blob/main/src/dpx.imageio/libdpx/DPXHeader.h). Linear transfer with explicit normalized reference quantities and Rec.709 primaries maps to linear sRGB; other color interpretations remain unspecified. Log/density transforms, video ranges/YCbCr, signed/float samples, packed 10-bit, 10-bit luma, multiple elements, RLE, alpha interpretation qualification and live viewer coverage remain pending. `scripts/generate-dpx-fixtures.py` authors 24 CC0 fixtures, then uses FFmpeg native-depth readback as an independent oracle. ABGR16 channel permutation, orientations and nonzero padding have additional authored checks; they are not independent interoperability proof.

Cineon uses its own V4.5 header and magic in either byte order. Channel descriptors, dimensions and bit depth must agree; this implementation supports pixel-interleaved unsigned gray/RGB 8/16-bit byte/word storage and 10-bit RGB longword-left/right storage. Validated payloads use the DPX sample engine through a bounded header adapter, retaining common normalization, eight orientations and explicit line/image padding. Format definitions follow [OpenImageIO's Cineon header](https://github.com/AcademySoftwareFoundation/OpenImageIO/blob/main/src/cineon.imageio/libcineon/CineonHeader.h) and [sample reader](https://github.com/AcademySoftwareFoundation/OpenImageIO/blob/main/src/cineon.imageio/libcineon/ReaderInternal.h). Printing-density/Rec.709 descriptors, image sense, primaries and gamma do not establish a complete display transform here: decoded samples remain unspecified-color and cannot silently display as sRGB. Film-log/density transforms, metadata retention, 12-bit/tight packing, packing flags, line/channel interleave, channel reordering, signed samples and live viewer qualification remain pending. Twelve CC0 fixtures use OpenImageIO 3.2.1.1 native readback through float EXR; original 10-bit codes are recovered from its bit-expanded values, then normalized with the library's documented rounding policy. This tests sample storage, not film appearance.

Softimage PIC uses the 0x5380f634 magic and PICT identifier, routed before extension-based RAW selection. The bounded decoder accepts version 1, square pixels and full-frame fields with complete RGB packets and optional alpha. Raw, pure-RLE and mixed-RLE streams support 8/16-bit big-endian samples and mixed packet precision, promoting 8-bit channels exactly to 16-bit when needed. Channel overlap, unknown masks/encoding flags, zero runs, cross-row runs, truncated streams and trailing payload are rejected. Storage definitions follow [OpenImageIO's Softimage reader](https://github.com/AcademySoftwareFoundation/OpenImageIO/blob/main/src/softimage.imageio/softimageinput.cpp). Twelve authored CC0 fixtures use OpenImageIO 3.2.1.1 native sample readback without automatic premultiplication; they include separate RGB/alpha packets, 16-bit alpha, mixed precision, literals, short repeats and extended repeats. Import currently assumes sRGB because the format lacks a qualified color profile interpretation. Field/aspect handling, incomplete channel layouts, other type flags/versions, alpha interpretation beyond these fixtures and live viewer qualification remain pending.

Folder thumbnail cache entries now retain the decoded source fingerprint plus the canonical WAL PCX palette fingerprint. Folder-strip rebuild revalidates these dependencies, releases stale GPU tile handles and queues replacement decodes. The worker compares fingerprints before/after decoding; the cache records that verified snapshot rather than rereading dependencies at publication. Palette replacement, deletion, creation, source changes, in-flight snapshot mismatch and LRU cleanup have application regressions. A real thumbnail-loader check confirms changed PCX colors and transparent-index compositing. Fingerprints use the existing metadata/sampled BLAKE3 contract, not full-file immutable snapshots. This is refresh-triggered invalidation; automatic filesystem watching and live window qualification remain pending.

Wavefront RLA uses a 740-byte header and absolute scanline offsets. The decoder validates revision 0/0xFFFE, ordered full/active windows with the active region contained within the full window, unfielded gray/RGB with optional single matte, and 8/16-bit integer storage. Each channel record has a 16-bit length and separately encoded MSB/LSB byte planes; literal -128 means 128 data bytes, not a PackBits no-op. Records may appear in any physical row order; overlap, gaps, incomplete planes, trailing record/image data, fields, auxiliary channels and linked subimages are rejected. Definitions follow [OpenImageIO's RLA reader](https://github.com/AcademySoftwareFoundation/OpenImageIO/blob/main/src/rla.imageio/rlainput.cpp). Fourteen CC0 files independently compare native samples through OpenImageIO 3.2.1.1; the oracle retains stored channel values without automatic premultiplication.

RLA alpha association is exporter-dependent: [Autodesk exposes a premultiplied-alpha option](https://download.autodesk.com/us/3dsmax/2012help/files/GUID-CF062571-6C55-4DC6-A48B-9B659CE42CF-2917.htm). Opaque RLA can use the general API with unspecified color. Matte-bearing files require an explicit alpha mode through `DecodeRequest::rla_alpha_mode` or `decode_rla_with_interpretation(request, RlaAlphaMode, RasterColorSpace)`; routing without that choice returns a typed interpretation-required error instead of guessing. Straight interpretation retains integer samples, and premultiplied interpretation unassociates into float samples, preserving composition over a background in an independent-readback check. Zero-alpha emission is explicitly rejected until associated-alpha rendering exists. Gamma/chromaticity metadata retention/automatic transforms, producer qualification of active-window coordinates, other integer bit depths, mixed float/integer groups, arbitrary auxiliary/matte channels, image selection, graphical interpretation controls and live viewer qualification remain pending. CLI alpha/color settings are available. Native decoding normalizes the full window to a top-down canvas, places each active row at `full_top - active_bottom - row`, and fills the outside with transparent black. Signed coordinate translation and memory admission are tested; OpenImageIO confirms window sizes but uses a different vertical-origin conversion, so offset full-canvas pixel parity with that oracle is not claimed.

WAD3 routes by signature and decodes the base mip of uncompressed type-0x43 mip-texture entries. `image_index` counts only mip-texture entries, skipping service lumps. The directory, lump extents/non-overlap, names, four mip offsets, 256-color palette and up to three zero alignment bytes are bounded/validated. RGB is imported with an explicit sRGB assumption; a `{` texture-name prefix applies the GoldSrc cutout convention to index 255, preserving its stored RGB. Header contracts follow [Valve's WAD library](https://github.com/ValveSoftware/halflife/blob/master/utils/common/wadlib.h) and [id's WAD definitions](https://github.com/id-Software/Quake/blob/master/WinQuake/wad.h).

`generate-wad-fixtures.py` authors two CC0 archives with a service lump and two textures, with reversed physical texture order in one archive. vgio 1.3.0 independently reads directory entries and mip indices; Pillow expands the authored palette and provides four exact RGBA oracles. The palette and cutout policy are authored expectations, not an independent complete WAD3 palette/engine decoder. Misleading camera extensions retain selected-texture routing; DCX/WAD page admission now follows content instead of an early extension-only index guard. WAD2 external palettes, Doom IWAD/PWAD images, compressed and other texture lump types, animation chains, mip selection, archive navigation UI qualification, color/engine-native interoperability and full live viewer qualification remain pending.

Viewer selection now carries `image_index` through the foreground latest-wins request queue and prepared-raster event. `--image-index` selects a zero-based DCX page/WAD3 texture for startup and inspection, and bracket keys request previous/next images within the current file. Successful GPU upload commits the displayed index; failed/out-of-range decode retains the previous index and pixels. File navigation resets the index, and nonzero sensor requests are rejected before RAW cache lookup. Application tests cover selected display-prepared pixels/alpha, out-of-range pages, CLI parsing, pending-index propagation and resetting the index for a new file. Live keyboard/window verification and a direct image-selection menu remain pending.

DCX and WAD3 now return validated selected-image index/count on `DecodedRaster`, counting only selectable WAD3 texture entries. Both direct and ICC color-preparation paths preserve this metadata. The application commits the count alongside the displayed index, shows a one-based position/total in the title, and prevents bracket requests beyond the decoded container bounds. Single images default to index 0/count 1. Independent DCX pixels under a misleading RAW extension, ICC metadata preservation, invalid selection bounds and application edge navigation have regression checks. CLI out-of-range requests still produce typed decoder errors; native window verification remains pending.

NRRD uses `decode_nrrd` for raw scalar values and retained header fields/custom key-value metadata. Axis 0 varies fastest; axis 2 is selected with `image_index`, and image count survives explicit grayscale windowing. Binary sample units and float32 bits are preserved without a color assumption. `NrrdImage::windowed(minimum, maximum)` explicitly maps finite samples to linear grayscale; nonfinite values require a separate missing-value policy. Three-dimensional arrays require scalar domain/space kinds. Independent pynrrd 1.1.3 readback covers 32 selected planes in raw, gzip and text storage, including both binary endian orders. Detached arrays, vector/color axes, other numeric types, physical-space reorientation, automatic window estimation and live viewer qualification remain pending. [NRRD format specification](https://teem.sourceforge.net/nrrd/format.html).

MRC2014 uses `decode_mrc` for scalar sections in storage column/row order and `image_index` for the section. The complete 1024-byte header and extended header are retained, including physical-axis permutation and origin; no physical-space reorientation or handedness is inferred. Both machine byte orders and scalar modes 0/1/2/6/12 are supported, with exact integer/float32 values and IEEE half-to-float32 conversion (signaling NaNs become quiet). `MrcImage::windowed` shares the explicit finite grayscale window with NRRD, preserving selection metadata. The general display route refuses unspecified scientific color. Complex modes 3/4, packed mode 101, older headers, compressed wrappers, typed exporter metadata, physical-coordinate views and live UI slice/window controls remain pending. [CCP-EM MRC2014 specification](https://www.ccpem.ac.uk/mrc-format/mrc2014/).

Scientific display now uses `ScalarWindow` and `decode_raster_with_window`: finite increasing raw-unit bounds, selected NRRD/MRC plane (FITS uses physical-unit windowing), cancellation checks during window conversion, explicit linear grayscale and preserved image count/index. `--window MIN MAX` applies the same policy to viewer startup and CLI inspection; ordinary color/sensor images reject this option. The foreground request/event carries the window with the selected slice, and successful GPU upload commits it. Bracket navigation retains bounds; opening another file clears them. Up/Down shifts center and PageUp/PageDown adjusts width, with finite-range validation. Public API/app checks cover raw-to-grayscale samples, CLI bounds, cancellation, slice selection and queue isolation. Existing GPU readback tests verify the linear-gray display pipeline; direct native keyboard/window evidence remains pending.

FITS uses `decode_fits` and `FitsSamples` to retain native unsigned 8-bit, signed 16/32/64-bit and IEEE float32/64 selected-plane samples. Original 80-byte cards retain WCS descriptors, units and repeated history; physical sky projection is not inferred. Selection flattens primary/IMAGE sections, skipping empty HDUs and bounded ASCII/binary tables (including binary heaps). BSCALE/BZERO apply to physical values; BLANK is tested before scaling and maps to an undefined value. Float BLANK is ignored in favor of IEEE nonfinite samples. Conversion to the common float32 raster rejects precision loss; explicit physical-unit windowing uses float64 and preserves native samples. Integral scaling/offset uses checked i128 to preserve small differences after large offsets. Wide integer fractional scaling that would lose physical precision is explicitly rejected. Compressed image conventions, random groups, dimensions above three, checksum verification, WCS projection, automatic missing-value policy and native live viewer qualification remain pending. [NASA FITS required keywords](https://fits.gsfc.nasa.gov/users_guide/users_guide/node21.html), [scaling and BLANK semantics](https://fits.gsfc.nasa.gov/users_guide/users_guide/node22.html).

FITS qualification executed: 22 selected-plane Astropy native/physical/window comparisons, exact wide-integer offset regression, malformed keyword/order/padding/bounds checks, float-special bit preservation, misleading RAW-suffix routing and common scientific window API. The application loader test includes selected FITS samples. Built CLI inspection with an explicit window succeeded for float64 and a later IMAGE HDU after a heap-bearing table; unwindowed int64 precision loss was rejected. Full decoder tests passed 452 cases (3 ignored), application tests passed 59. These results do not qualify WCS/CFITSIO interoperability, compressed conventions or a live native window.

PVR v3 uses `decode_pvr` for retained metadata and the common raster router for selected mips. Ordinary byte-aligned normalized 8/16-bit and float32 channel descriptors support RGB(A), BGRA, luminance/alpha and sparse RGB channels; declared sRGB/linear transfer is honored. Both byte orders are handled for ordinary data. Metadata orientation flips X/Y, padding is skipped, custom blocks are retained and uniform per-channel type overrides are applied. Unknown interpretation-changing metadata and nonzero texture borders are rejected. PVRTC1 RGB/RGBA 2/4 bpp uses bounded power-of-two extents, format minimum padding and cropped small mips. Linear premultiplied alpha is converted to straight float; zero-alpha emission and unqualified sRGB premultiplication are explicit unsupported cases. Other compressed codecs/PVRTC2, big-endian compressed payloads, packed/mixed channel widths/types, legacy PVR, cube/array/volume selection and live window qualification remain pending. [PowerVR header specification](https://docs.imgtec.com/specifications/pvr-file-format-specification/html/topics/pvr-header-format.html), [metadata specification](https://docs.imgtec.com/specifications/pvr-file-format-specification/html/topics/pvr-metadata.html).

PVR qualification executed: all 58 selected-mip native-byte oracles across 18 reproducible files, three linear-alpha composition comparisons, malformed header/metadata/payload bounds, invalid selection, mixed compressed-type rejection, HDR association and misleading RAW-suffix routing. The application preparation test includes a selected big-endian 16-bit mip. Full decoder tests passed 457 cases (3 ignored), application tests passed 59; application build, formatting and diff checks passed. The PowerVR SDK oracle independently validates raw PVRTC decompression, not the complete container reader. Native window qualification remains pending.

PVR CLI verification completed after macOS loader startup: ordinary big-endian selected mip 1 reports 2x1 RGBA16, index 1/count 3, sRGB; PVRTC selected mip 2 reports 8x4 RGBA8, index 2/count 4, sRGB. All seven PVR tests passed, including rejection of every truncated prefix across all fixtures and cancellation before I/O/header validation. Live native-window qualification remains pending.

DDS mip qualification executed: 36 standalone-Pillow sample comparisons across nine four-level chains, selected transfer/alpha checks, common-router and application preparation tests, malformed count/pitch/truncation/trailing storage and out-of-range selection checks. Regeneration is byte-identical and source hashes/oracle lengths were verified. Full decoder tests passed 461 cases (3 ignored); application tests passed 59. These are selected 2D texture checks, not cube/volume/array or full format qualification.

KTX1 selects stored 2D mip levels through `image_index`, validates every declared level before returning any selection and preserves index/count through display preparation. Sixty native-byte fixtures cover fifteen four-level chains, ordinary 8/16/float32 RGB(A), both byte orders, padding, orientation and BC1/2/3. Authored sample oracles and independent DDS block oracles do not qualify a full independent KTX loader. A zero level-count field exposes only the stored base image; automatic generation of missing levels is not implemented. Cube/array/volume selection, other storage and live-window qualification remain pending. [Khronos KTX1 specification](https://registry.khronos.org/KTX/specs/1.0/ktxspec.v1.html).

DDS CLI inspection confirmed mip index 2/count 4, 2x1 RGBA8, sRGB for the DX10 RGBA32 chain. Live native-window navigation remains pending.

KTX1 mip qualification executed: all sixty native-byte comparisons pass, every truncated chain prefix is rejected, out-of-range indices fail explicitly and misleading sensor suffixes route by KTX signature. The application loader preserves selected big-endian HDR float32 samples/alpha and count. Full decoder tests passed 464 cases (3 ignored), application tests passed 59; application build, formatting and diff checks passed. CLI inspection confirmed selected mip 2/count 4, 2x1 RGBA32F, LinearSrgb.

KTX2 stored-mip selection qualification executed: 96 native-byte contracts across 24 four-level ordinary/DEFLATE chains, 8/16/float32 precision, orientation, transfer and selection metadata. All lower-level container ranges and declared sizes are validated even when the base level is selected; unselected compressed pixels are decoded only on selection; malformed/truncated storage and out-of-range indices reject explicitly. Full decoder tests passed 466 cases (3 ignored), application tests passed 59, and application build/format/diff checks passed. The original baseline generator was refactored to expose its container builder without generating files on import; baseline regeneration and source hashes were rechecked. Built CLI inspection confirmed selected DEFLATE mip 2/count 4, 2x1 RGBA32F, LinearSrgb. These are authored container/sample contracts; independent libktx interoperability and live-window qualification remain pending.

APNG uses `decode_apng` for selected presentation frames, rational millisecond delay, number of plays (zero means infinite) and retained source color declaration. The common raster API selects the animation frame through `image_index`, excluding a separate default preview. PNG static decoding supplies raw 8-bit frame pixels; existing color preparation transforms each frame to linear sRGB before the owned straight-alpha float compositor applies SOURCE/OVER and NONE/BACKGROUND/PREVIOUS disposal. The presented raster is RGBA32Float LinearSrgb. CRCs, control count, sequence numbers, frame geometry, delay and complete chunk bounds are checked. Source unknown/unsupported color interpretation is an explicit color error; nontrivial EXIF orientation and 16-bit animation are unsupported until qualified. Frame count is capped at 4096 and a 512 MiB budget bounds four canvas-sized working buffers. Compressed frame payloads borrow source storage. Decoding starts at the latest independent full-canvas SOURCE frame and stops at the selected presentation; unused compressed pixels are checked when needed, while all chunk CRCs and controls are always validated. CRC checks are cancellable at 64 KiB intervals.

Three authored APNG files provide twelve linear-light mathematical oracles and separate FFmpeg 9.0.2 compatibility presentations. Native compositor alpha fixes were necessary: image crate returned 254 over an opaque destination, and Pillow 191; correct opacity remains 255. FFmpeg also keeps opaque alpha but blends encoded RGB, so its presentation pixels cannot prove normative linear-light color. Delay/loop values, default-image exclusion and disposal have authored controls and separate readback evidence. Timed UI playback/loop scheduling, 16-bit/palette/interlace qualification and full independent color-aware animation interoperability remain pending. [W3C PNG/APNG composition specification](https://www.w3.org/TR/png-3/#11fcTL).

APNG verification executed: all twelve linear presentation oracles (2e-7 sample tolerance), rational delay/repeat values, selected display preparation, every truncated source prefix, corrupt CRC/trailing data, valid-CRC malformed counts/sequences/geometry/disposal/blend and cancelled-before-I/O requests. Source hashes and linear oracle sizes pass; full regeneration is byte-identical. Full decoder suite passed 469 cases (3 ignored), application suite passed 59; formatting and diff checks passed. This verifies selected presentation semantics, not running animation playback.

Loading/display preparation is the current performance priority. See [measured raster loading results](RASTER_LOAD_PERFORMANCE.md) for paired CPU timings, exact pixel fingerprints, lazy decoding scope and the remaining first-presentation measurement.

GIF selected presentation: `decode_gif` returns the raster, exact centisecond delay converted to milliseconds (including zero), and optional Netscape repeat count (`None` once, `Some(0)` infinite, positive repeats after the first play). Structural inventory validates all block bounds, image rectangles, control/disposal flags and frame count (maximum 4096), without inflating later frames. Image/GIF decoding composes only through the selected frame, using transparent canvas/background semantics consistent with the image crate and authored transparent GIF fixtures. Plain-text rendering, user-input controls and reserved disposal modes reject explicitly. Opaque logical-background semantics, application color extensions and timed UI playback need additional qualification. [GIF89a specification](https://giflib.sourceforge.net/gifstandard/GIF89a.html).

Three CC0 animations generated by `scripts/generate-gif-animation-fixtures.py` provide twelve Pillow-decoded presentations, partial rectangles, KEEP/BACKGROUND/PREVIOUS, transparency, zero/nonzero delays and finite repetition. `gif-animation-manifest.tsv` records selection, dimensions, delays, repetitions, oracle and source hash. Common raster routing preserves selected index/count even with misleading file extensions.

GIF verification executed: twelve independent selected presentations, exact delay/repeat values, dimensions/index/count and source hashes/oracle sizes; every truncated prefix, invalid canvas, trailing data and out-of-range selection reject. A corrupt final LZW stream leaves frame zero decodable and rejects selection of that final frame. Strong GIF signature permits selected routing under a RAW suffix. Application display preparation verifies selected frame 3/count 4 and expected linear red/blue samples. Full decoder suite passed 474 tests (3 ignored); application suite passed 59; formatting and diff checks passed. Live window playback remains pending.

WebP animation uses bounded RIFF/VP8X/ANIM/ANMF inventory, selected frame count/index, exact 24-bit millisecond delays and 16-bit total-play count (zero infinite). All outer and nested chunk ranges/padding, rectangles, reconstruction ordering and ICC declarations are checked. Individual VP8/VP8L and ALPH+VP8 surfaces decode through the image WebP codec, then existing color preparation converts source RGB to linear sRGB before the shared straight-alpha compositor applies SOURCE/OVER. Declared BGRA background is converted through the same source color pipeline and used for initial canvas/disposal. Selected output is RGBA32F LinearSrgb; source ICC declaration remains in `WebpImage`. Nontrivial animated EXIF orientation rejects until qualified.

Compressed frame storage borrows the input, with at most 4096 frames and 65536 chunks per inventory. Four canvas-sized linear working buffers fit the 512 MiB output budget. Decoding stops at selection and restarts at the latest full-canvas SOURCE frame, avoiding unused dependencies. Unneeded compressed pixels are validated when needed/selected, while structural ranges remain checked across the full container.

`scripts/generate-webp-animation-fixtures.py` creates four CC0 animations with sixteen selected presentations. libwebp `cwebp` encodes raw source frames; independent `dwebp` readback supplies source samples, including lossy RGB with lossless alpha. Python float64 linear-light equations supply presentation oracles; these are authored mathematical expectations rather than independent color-aware animation-decoder readback. Cases cover partial rectangles, SOURCE/OVER, disposal to transparent/partly transparent declared background, zero/nonzero delays and zero/finite plays. [WebP container specification](https://developers.google.com/speed/webp/docs/riff_container). Full codec variants, independent full-animation color interoperability and timed live viewer playback remain pending.

WebP animation verification executed: all sixteen native-source/linear presentation contracts (2e-7 tolerance), exact delays/plays and selected dimensions/count, source hashes and oracle lengths. Full fixture regeneration is byte-identical with libwebp 1.6.0. Every truncated prefix, malformed selected compressed payload and out-of-range index rejects; corrupt earlier pixel data is skipped for an independent later full-canvas SOURCE frame, and corrupt later pixels do not block the first frame. RGB ICC conversion and animated EXIF refusal are tested. Application preparation verifies selected OVER frame/count and linear-light red/green with opaque alpha. Full decoder suite passed 477 cases (3 ignored), application suite 59; formatting/diff checks passed. No new dependency or lockfile change. Timed live-window presentation and playback remain pending.

Standalone JNG follows [JNG 1.0](https://www.libpng.org/pub/mng/spec/jng.html): validated PNG-style chunk CRC/bounds, first JHDR, interleaved JDAT/IDAT/JDAA and terminal IEND. 8-bit JPEG RGB/gray baseline/progressive streams decode through the existing JPEG codec; separate PNG grayscale alpha retains 1/2/4/8/16-bit storage scaling, or JPEG grayscale supplies lossy 8-bit alpha. For alpha16 the result is RGBA16, with JPEG RGB expanded exactly by 257 and alpha retained without rounding. Alpha is straight and bypasses color conversion. Declared JPEG/alpha dimensions and channels must agree.

Outer sRGB/iCCP/gAMA/cHRM declarations use existing PNG color interpretation independently of the alpha stream; absent declarations produce AssumedSrgb, unknown gamma/primaries remain unspecified. Embedded JPEG metadata does not replace outer color interpretation. Dimensions are capped at 65536 per axis and a 512 MiB working/output bound, with cancellable chunk CRC/conversion. Unknown critical chunks, 12-bit/dual-depth JSEP, unsupported storage, missing/mixed alpha streams, CRC faults and trailing data reject explicitly. Wider JPEG/JNG variants, MNG embedding, independent full-JNG interoperability and live window opening remain pending.

`scripts/generate-jng-fixtures.py` authors eight CC0 containers with independent Pillow JPEG color/JPEG-alpha readback plus authored lossless alpha samples. Native RGBA16 oracles retain adjacent alpha values. Fixtures cover gray/RGB, baseline/progressive color, split/interleaved JPEG chunks, all PNG alpha depths and JPEG alpha. Manifest source hashes, oracle lengths and byte-identical regeneration pass.

JNG verification executed: eight independent JPEG/native-alpha sample contracts (JPEG tolerance two byte levels, lossless alpha exact), outer sRGB/assumed/RGB ICC display semantics, adjacent alpha16 preservation, source hashes/oracle sizes and byte-identical regeneration. Every truncated prefix, CRC/trailing fault, valid-CRC malformed header and unsupported selection rejects. Source JPEG channels/depth/progressive mode must agree with JHDR; four-component CMYK cannot silently enter RGB routing. Application display preparation confirms 8x3 linear float output with unchanged 32768/65535 alpha. Full decoder suite passed 480 cases (3 ignored); application suite 59; formatting/diff checks passed. No new dependencies or lock change. Full live-window/JNG interoperability qualification remains pending.

JPEG-LS uses pinned Rust CharLS bindings 0.4.2, charls-sys 2.4.5 and bundled static CharLS 2.4.2 (BSD-3-Clause). No runtime codec command is required. Header dimensions/channels/depth are inspected before native pixel allocation; gray/RGB 2–16-bit samples are supported, with a 512 MiB working/output bound. Planar native output is mapped by component plane; line/sample native output is pixel-interleaved. Source depth, MAXVAL, NEAR and interleave metadata are retained by `JpegLsImage`. Values are expanded with integer rounding to full-range RGBA8/RGBA16, preserving every source code injectively; 16-bit full-range samples remain exact. Output alpha is opaque. Undeclared gray/RGB uses explicit AssumedSrgb import; fragmented RGB ICC bytes enter the existing color pipeline. Unsupported SPIFF color interpretation, channel counts, mixed scans/presets and point transforms reject explicitly; full SPIFF/custom-preset/gray-ICC qualification remains pending.

Complete JPEG-LS marker/entropy inventory checks exact terminal EOI, scan count/order and uniform scan parameters, including JPEG-LS's MSB-zero bit stuffing and restart-marker handling. Concatenated images/trailing data reject. Cancellation is checked before/after native decoding, during inventory and sample conversion; native decode itself does not expose an in-call interruption hook.

`scripts/generate-jpegls-fixtures.py` authors 29 CC0 files across gray/RGB, 2/4/8/10/12/16-bit codes, all three layouts and two near-lossless cases. Three streams are FFmpeg-encoded; other variants are generated by a test-only CharLS encoder. Thirteen contracts have independent FFmpeg native readback (gray storage shifted back to source bit alignment); sixteen use authored lossless source-code expectations. FFmpeg 9.0.2 produces inconsistent non-8-bit RGB samples and zero sample-interleaved RGB without an error, so those results are not interoperability evidence. Wider RGB precision/sample-interleave needs another independent decoder. [CharLS source and API](https://github.com/team-charls/charls).

JPEG-LS verification executed: 29 sample/metadata contracts, fragmented out-of-order RGB ICC reassembly, duplicate ICC rejection, cancellation before I/O, all truncated prefixes, oversized dimensions, trailing/concatenated sources. All source hashes/oracle sizes pass; full regeneration is byte-identical (59 files). Application preparation checks 9x5 gray16, an explicit assumed-color flag, linear gray and opaque alpha. Full decoder suite passed 483 tests (3 ignored), application suite 59; format/diff checks passed. Dependency licenses/bans/sources pass; the existing target-lexicon Apache-2.0 WITH LLVM-exception requirement was added to the allow list. Native RAW semantic lock digest is synchronized with the updated lockfile. Cold startup, first GPU presentation, full independent JPEG-LS interoperability and live window qualification remain pending.

## JPEG-XR implementation boundary

JPEG-XR (`.jxr`, `.wdp`, `.hdp`) uses pinned `jpegxr 0.3.1` with statically bundled JXRLib. Accepted full pixel GUIDs cover gray/RGB/BGR and ordinary/associated RGBA UINT8/UINT16 and HALF/FLOAT layouts, including padded RGB. UINT samples remain RGBA8/16; HALF is expanded exactly to float32 and FLOAT stays float32, retaining negative/HDR RGB. Explicit ICC wins; absent ICC, UINT defaults to sRGB and float to linear sRGB following [Microsoft WIC native pixel color conventions](https://learn.microsoft.com/en-us/windows/win32/wic/-wic-codec-native-pixel-formats). Packed/fixed-point/RGBE/CMYK/N-channel layouts, interleaved alpha and multiple image directories currently reject explicitly.

Before native decoding, the container validates typed tag bounds, duplicates, dimensions/allocation limits, full GUID, orientation and disjoint main/alpha ranges, then checks both codestream headers. The native wrapper does not expose alpha mode, so a small synthetic gray container borrows the separate alpha codestream and decodes it independently; alpha sample bytes are inserted into the native RGBA output. No compressed input copy is needed. A borrowed reader clears the container orientation for JXRLib's low-memory decoder, and an owned typed RGBA permutation implements all eight orientations. Native decoding itself has no cancellation hook. The narrowly bounded compatibility case `ALPHA_BYTE_COUNT == file length` accepts JXRLib 1.1's absolute-end alpha count; canonical fixtures otherwise store the actual byte count.

`scripts/generate-jpegxr-fixtures.py` generates 21 CC0 sample contracts using imagecodecs 2026.8.16 / JXRLib 1.1. Non-alpha native samples are checked against independent `openreadout-jpegxr 0.1.0` using the checked-in test-only helper `scripts/jpegxr-fixture-oracle.rs`; its Rust 1.91 requirement does not change the project's Rust 1.89 MSRV. Orientation is an authored coordinate permutation of independently decoded samples, with native flip readback checked for codes 0–3. Integer alpha source arrays round-trip exactly; float alpha uses JXRLib readback and is not independent interoperability evidence. General lossless encoding, every supported GUID, arbitrary tiles/window margins, malicious entropy and full color/interop/live window behavior remain qualification work. First GPU presentation and cold startup performance remain unmeasured.

JPEG-XR validation: 21 exact typed sample/orientation contracts; every truncated prefix of four representative containers rejects during metadata checks; malformed dimensions, offsets/counts, orientation, GUID and field types reject. The full decoder suite passes 486 tests (3 ignored), including the unchanged exhaustive HALF oracle. Misleading RAW-extension routing is covered. The application suite passes 59 tests with a new JPEG-XR HDR/separate-alpha preparation case. All 43 fixture/manifest files regenerate byte-identically; hashes and typed oracle sizes pass. Dependency license/bans/sources and diff whitespace checks pass. The native semantic lock digest matches Cargo.lock. These checks do not constitute full JPEG-XR or all-100-format qualification.

## Aseprite implementation boundary

Aseprite (`.ase`/`.aseprite`) has an owned bounded parser and selected-frame renderer, with no new runtime codec dependency. RGBA, grayscale-alpha and indexed cel storage stays borrowed until a visible selected cel is needed. Both raw and zlib images, signed positioning/clipping, forward/backward linked storage and chains are supported. The link graph resolves once in place and rejects missing targets and cycles; current cel position/opacity/z remains independent of its image source. Every frame/chunk range and known structural field is checked, while unrelated compressed streams are checked only when rendered. The first full-canvas RGBA cel at full opacity inflates directly into the final canvas; other cels use a bounded temporary buffer. Inflation processes at most 64 KiB input/output per iteration, checks cancellation/progress, exact sample count, checksum and stream termination.

Normal source-domain eight-bit alpha blending matches Aseprite rounding and signed RGB truncation, before common display color preparation. Visibility, background transparent-index rules, cel/layer opacity validity, native z-order tie breaking, group isolation and group opacity are handled. Palette changes apply through the selected frame and recolor linked indexed storage; modern palettes override legacy chunks, including six-bit legacy color expansion and the 256-entry zero-count form. Declared sRGB/linear sRGB and RGB ICC reach the common color pipeline; absent profile uses explicit assumed sRGB. The public `decode_aseprite` retains delay, layer names and tag ranges/direction/repeat metadata; common APIs retain selection/count. Layer/tilemap modes beyond normal, external resources, precise/scaled cel bounds, non-square pixel aspect, custom gamma, gray ICC, live tag playback and wider interoperability remain qualification work. Unknown rendering chunks reject explicitly. Structural limits include 4096 layers, 64 group levels, 1,048,576 chunks and a 512 MiB canvas/cel budget, separately from the shared 2 GiB input cap. Semantics follow the [Aseprite file specification](https://github.com/aseprite/aseprite/blob/main/docs/ase-file-specs.md) and [native normal blender](https://github.com/aseprite/aseprite/blob/main/src/doc/blend_funcs.cpp).

`scripts/generate-aseprite-fixtures.py` authors 21 CC0 source files and 63 selected-frame contracts. The standalone `scripts/aseprite-fixture-oracle.rs` uses pinned `asefile 0.3.8` outside the runtime workspace. Thirty-nine contracts have independent full-file raw sample readback; twenty-four are explicitly authored source/composition expectations. The reference decoder ignores the old invalid-layer-opacity flag and lacks the tested ICC/linear-gamma, group composition, moving/chained/forward-link and palette-update semantics, so it is not evidence for those cases. Full decoder verification passes 491 tests (3 ignored), including exact pixels, metadata and both common APIs for all 63 contracts. Truncated prefixes, malformed headers/fields, cyclic links, unsupported blending/gamma, cancellation before I/O, RAW-suffix content routing and selection without unrelated inflation are covered. Fixture hashes and sizes pass; all 85 source/oracle/manifest files regenerate byte-identically. Native semantic lock digest remains synchronized. Live window/physical first-presentation and full all-100 qualification remain unproven.


Single-part EXR color metadata now uses a bounded header reader (1 MiB, 4096
attributes, 255-byte names/types). Explicit Rec.709/D65 `chromaticities` or
`colorInteropID=lin_rec709_scene` select linear sRGB, which preserves HDR floats
without a transfer curve or clipping. Unknown IDs override legacy chromaticities;
missing declarations remain linear RGB with unqualified primaries. Conflicting
Rec.709 declarations/ACES flags, malformed types, duplicates and non-finite
coordinates return errors. Non-D65 adopted neutrals remain unqualified. Deep and
multipart color selection is explicitly unsupported. Other gamut/white-point
transforms, luminance mapping and complete metadata retention remain pending.
The rules follow [OpenEXR standard attributes](https://openexr.com/en/latest/StandardAttributes.html)
and [Color Interop recommendations](https://github.com/AcademySoftwareFoundation/ColorInterop/blob/main/Recommendations/01_TextureAssetColorSpaces/TextureAssetColorSpaces.md).
A real independently generated uncompressed EXR fixture is extended with an
authored Rec.709 attribute and relocated absolute scanline offsets in the test;
its decoded sample bits match the original oracle-qualified fixture exactly.
Display preparation shares the managed float allocation, retains values above
one, and releases its reservation after the final owner drops. This is library
color admission evidence, not physical HDR presentation qualification.


Native DICOM Part 10 scalar decoding supports Explicit VR Little Endian
(`1.2.840.10008.1.2.1`), MONOCHROME1/2 and 8/16-bit allocated integer pixels.
Bits Stored/High Bit and signed representation are validated; unused high bits
are masked, including signed 12-bit values in 16-bit words. Native pixel length
includes the final even-byte padding; frame selection excludes padding. File
meta group length/version/required UID fields, element VR lengths/reserved bytes,
ordered unique tags, sample layout and all frame ranges are validated before
pixel publication. Elements are capped at 65536 and output at 512 MiB; source
reads and selected RGBA32F output participate in managed admission. Metadata
inventory allocations are bounded but outside the managed pixel/source budget.
Cancellation is checked in metadata scanning and pixel conversion.

Absent rescale tags use stored units; both slope/intercept are required when
rescale is declared. Samples use finite f64 rescale then bounded f32 storage.
Without `--window`, values remain unspecified-color scalar units and cannot
silently display as sRGB. With explicit minimum/maximum bounds, values are
linearly normalized/clamped in rescaled units and MONOCHROME1 presentation is
inverted. This is the existing scientific-window contract, not DICOM's automatic
Window Center/Width/VOI/presentation-state pipeline. LUT/sequences, pixel padding
policy, color images, float pixel data, compressed/implicit/big-endian syntax,
patient-space orientation/spacing and complete IOD conformance remain pending.
Unknown syntax/storage never falls back to a preview. DICM at offset 128 takes
priority over TIFF-like preamble bytes in classification and decoding.

Rules follow [DICOM PS3.5](https://dicom.nema.org/medical/Dicom/current/output/html/part05.html).
Four authored fixtures from `scripts/generate-dicom-fixtures.py` use pydicom
3.0.2/numpy 2.2.6 for independent native pixel and modality-rescale readback.
They cover two signed/rescaled frames with nonzero unused high bits, adjacent
unsigned 16-bit values, MONOCHROME1 with odd native byte count, and TIFF-like
preamble routing. RGBA window oracles apply the explicitly authored generic
bounds, not a clinical VOI oracle. Every selected frame matches raw/rescale and
window sample bits, metadata and managed-budget lifetime. Every truncation,
unknown syntax, malformed bit layout, undefined length, rescale overflow and
cancellation rejects. Viewer preparation/cache tests preserve selected frames,
exact window keys and external managed ownership. These fixtures qualify pixel
subsets; they do not claim complete clinical IOD or all DICOM interoperability.

## NRW implementation prerequisites (2026-10-05)

The CC0 Coolpix P7800 sample (raw.pixls.us object 1425) is downloaded under
`target/qualification/nrw`; its 26,992,101 bytes match repository SHA-256
27ac09537dd0f75523864dcf221278e2db354d490534bc13b6eabdc031060e51.
The independent LibRaw 0.22.2 oracle successfully unpacks a 4032x3024 RGGB
mosaic. Source provenance, complete sensor digest, WB, black/white levels and
camera matrices are pinned in `tests/fixtures/nrw-qualification.json`.
This is oracle evidence only: native NRW decoding remains Pending. P6000 and
P7000 repository samples carry noncommercial licenses and are not adopted as
redistributable fixtures.

Diagnostic routing of an unchanged P7800 byte stream under a temporary NEF
filename first exposed CFAPattern stored as TIFF UNDEFINED. The Nikon reader
now accepts that byte-sized representation with unchanged cell count, known
color and 2x2 Bayer validation. A synthetic regression verifies RGGB cells and
rejects an invalid color; all 25 Nikon tests pass in `/tmp/rrrah-nrw-cfa.log`.
The real diagnostic now reaches pixel decoding and rejects the absent Nikon
curve tag, confirming that P7800's maker-note compression mode 7 needs a
distinct unpacked path rather than the current compressed-NEF algorithm.
This change does not enable NRW routing or qualify its pixels/color.

Full-source storage probe: the pinned P7800 TIFF is little-endian, declares
12-bit samples and compression 34713, but its single 24,385,536-byte raw strip
contains big-endian 16-bit samples. Converting all 12,192,768 values to little
endian matches the independent LibRaw sensor dump byte-for-byte. The observation
is pinned in `nrw-qualification.json` and `target/qualification/nrw/storage-probe.json`.
It supports implementing an explicit P7800 unpacked storage mode; it does not
justify inferring a mode from payload length for arbitrary Nikon cameras.

Native P7800 unpacking is now implemented behind exact model and Nikon
maker-note compression-mode 7 admission. Only the qualified single-strip,
12-bit-tagged, one-component layout is accepted; decoded storage remains
big-endian u16. Strip length/ranges and per-row cancellation are checked.
The diagnostic NEF route produces a sensor dump byte-identical to LibRaw;
resolved WB and camera matrices agree as well. All 25 Nikon unit tests pass.
Black/white metadata remains incorrect (native 0/4095 versus reference
3200/65504), so native NRW extension routing remains disabled and NRW Pending.
Level resolution and malformed-mode/strip regressions are required next.

P7800 levels now use the explicit LibRaw 0.22.2 camera profile: black 3200
([colordata.cpp](https://github.com/LibRaw/LibRaw/blob/0.22.2/src/tables/colordata.cpp))
and white 65504
([identify.cpp](https://github.com/LibRaw/LibRaw/blob/0.22.2/src/metadata/identify.cpp)).
Admission requires exact P7800 model, width 4032, declared depth 12 and Nikon
mode 7. A 128 MiB managed native diagnostic matches every sensor sample,
black/white levels and WB/matrices within 1e-6. All 25 Nikon tests pass.
NRW routing remains disabled until malformed mode/strip qualification is added.

Compact P7800 storage regressions now verify exact big-endian samples including
0, 3200, 65504 and 65535 with both maker-note byte orders. Unsupported mode 6,
declared depth 14 and a one-byte-short strip reject; per-row cancellation returns
the typed cancellation error. All 26 Nikon tests pass in
`/tmp/rrrah-p7800-malformed.log`. Extension routing still needs a dedicated NRW
admission guard so unknown Coolpix files cannot enter generic NEF defaults.

Native RAW routing now recognizes case-insensitive `.nrw` using a dedicated
guarded backend (ID 11, revision 1). Only COOLPIX P7800 is accepted; other
models reject before pixel decode. Nikon metadata/color parsing is reused,
and NEF semantic revision is bumped to 6 for the changed CFA/P7800 behavior.
The original CC0 `.NRW` source decodes under a 128 MiB managed budget and its
complete sensor dump matches LibRaw. Extension and unqualified-camera tests
pass with the full decoder suite (562 passed, four ignored). Format catalog,
viewer display classification and GPU qualification are still pending, so the
top-level NRW coverage entry is not yet promoted.

The common image router and supported-extension filter now classify `.nrw`
as sensor data, including uppercase names. Generic TIFF headers under this
extension remain sensor requests without reading/admitting the whole file
during classification, and malformed input cannot fall back to a TIFF preview.
This connects gallery filtering and RAW neighbour preload to the guarded native
backend. Decoder tests pass 563 cases (four ignored), and application binary
tests pass 104 (three ignored); logs are `/tmp/rrrah-nrw-image-routing.log` and
`/tmp/rrrah-nrw-app.log`. Real NRW GPU display qualification remains pending.

The real pinned P7800 source now passes forced Metal Apple M4 Max offscreen
readback: native sensor data and independent LibRaw sensor/reference metadata
produce identical 128x96 RGBA output, with nonblack RGB and opaque alpha.
Managed source allocation releases after the final owner drops. Reproduce with
`RRRAH_NRW_CORPUS=/absolute/path/to/target/qualification/nrw`
and `RRRAH_GPU_BACKEND=metal RRRAH_GPU_OPTIONAL=0 cargo test --locked -p rrrah --test nrw_readback -- --ignored --nocapture`.
Log: `/tmp/rrrah-nrw-metal.log`. The test is normally ignored because the
external corpus is required; this explicit run passes. NRW is promoted only
to the qualified P7800 subset, with physical presentation, other Coolpix
models and broad storage variants still pending.

The same real P7800 test now exercises the asynchronous RAW swap engine with
one disk entry and bounded queue/restore budgets. Original sensor ownership
is released after spill. A fully reserved restore budget rejects restoration
without corruption or entry removal; releasing it permits a managed restore.
Every restored sensor sample and the subsequent Metal RGBA readback match the
independent reference. One write, one successful read, zero errors, zero final
queue credit and zero final managed sensor credit pass in
`/tmp/rrrah-nrw-swap-metal.log`. This qualifies the tested P7800 swap/display
path, not full navigation or physical presentation.

P7800 metadata and storage now both require TIFF compression 34713 in addition
to maker-note mode 7. Synthetic compression 1 and 8 mutations reject in both
maker-note byte orders, preventing inconsistent tags from selecting the qualified
unpacked path. All 27 Nikon tests pass in `/tmp/rrrah-p7800-compression.log`;
the real NRW/swap Metal test passes again after this stricter admission.

The external-corpus GPU regression now runs both P7700 and P7800, with separate
independent sensor dumps and WB references. Both pass exact RGBA comparison
before and after real swap restoration on Metal Apple M4 Max. Each camera
also verifies pressure rejection/retry, one write/read, zero errors and final
CPU/queue credit release. Explicit run log:
`/tmp/rrrah-nrw-two-cameras-metal.log`. This extends the NRW qualified subset
to both models; other cameras/modes and physical display remain pending.

NRW remains Pending. Inspection of the native Nikon reader shows only NEF
extension routing and no explicit Coolpix storage modes. The current upstream
[rawler Nikon decoder](https://github.com/dnglab/dnglab/blob/main/rawler/src/decoders/nef.rs)
uses camera hints for interlaced 12-bit, MSB32 and unpacked storage; P7800
may require pixel byte order different from TIFF byte order. Its NRW white
balance branch uses maker-note tag 0x0014, an NRW signature and coefficients
at offset 1556 for version 0100 or offset 56 otherwise. These observations
are reference behavior, not qualification of our reader.

Implementation must obtain independently decoded real Coolpix fixtures,
identify each supported storage mode explicitly, bounds-check maker-note
coefficients and reject unknown versions. Color and WB must retain strict
admission, with no identity/unity fallback. Source bytes, intermediate buffers
and final mosaic require managed admission. Extension aliasing alone cannot
raise the supported-format count. The retrieved upstream source SHA-256 is
be187dad6204fead60ecddde424d57aae47c1064254285eafea5f2e3a97b8298; upstream main is mutable.

## Leica RWL initial qualification

The CC0 D-LUX (Typ 109) 1:1 sample, raw.pixls.us object 807, matches its
source SHA-256 and independently unpacked LibRaw 0.22.2 full sensor digest.
Native CFA, WB and camera transform agree; crop retains the source-tag policy
[8,8,3088,3088], distinct from LibRaw initial active area [0,0,3104,3104].
This crop contract is explicit in the shared corpus manifest. Case-insensitive
RWL routing and gallery extension filtering now use the existing RW2 backend
and content validation. All 14 corpus cases including local CR3 pass with
256 MiB managed admission; 563 decoder tests pass, four ignored. Evidence:
`target/qualification/rwl/corpus-report.json`, `/tmp/rrrah-rwl-corpus.log`,
`/tmp/rrrah-rwl-tests.log`. Real RWL GPU/display qualification is pending.

The real D-LUX Typ 109 source now passes Metal Apple M4 Max offscreen
qualification. Native and independent sensor/color reference produce identical
128x96 RGBA output before and after asynchronous RAW swap restoration. The
source-tag crop is asserted explicitly. Restore pressure preserves the disk
entry, retry succeeds, and all tracked sensor/queue ownership releases with
zero errors. Reproduce with `RRRAH_RWL_CORPUS=/absolute/path/to/target/qualification/rwl`
and `RRRAH_GPU_BACKEND=metal RRRAH_GPU_OPTIONAL=0 cargo test --locked -p rrrah --test rwl_readback -- --ignored --nocapture`.
Log: `/tmp/rrrah-rwl-metal.log`. Broader Leica cameras, physical window output
and photographic color accuracy remain unqualified.

## EIP package foundation

Capture One documents EIP as a standardized ZIP containing original RAW plus
settings and ICC/LCC assets:
https://support.captureone.com/hc/en-us/articles/360002478817-Pack-and-Unpack-EIP-files.
`inspect_eip` currently inventories a borrowed classic single-disk package.
It caps the central directory before ZIP parsing (64 entries, 64 KiB), validates
UTF-8 names (256 bytes), rejects traversal/absolute/drive names, duplicate names,
links, encryption, unsupported codecs and ambiguous/empty RAW candidates, and
bounds total declared uncompressed data to 1 GiB. Only stored/DEFLATE entries
are admitted. This inventory neither inflates payloads nor verifies their CRCs;
metadata allocation is bounded but not charged to a caller MemoryBudget.

The package is not yet a public image decode candidate. `read_eip_raw` now reserves managed storage before inflation and verifies
CRC/length through EOF; cancellation is checked between 64 KiB chunks. RAW identity
and recipe must include package semantics. Capture One adjustments, masks,
profiles and LCC need explicit interpretation or a typed refusal; sensor-only
inspection must not be presented as reproduction of the authored appearance.
No camera-produced EIP oracle is yet pinned, and EIP remains Pending.

EIP RAW extraction qualification uses independently authored Python zipfile
stored/DEFLATE packages containing pinned CC0 IIQ object 4366. The generator
`scripts/generate-eip-fixture.py` fixes timestamps/permissions; both packages
regenerate with identical SHA-256, recorded in `tests/fixtures/eip-package-corpus.json`.
All 21,495,886 extracted RAW bytes must equal the original source. Source package
storage is borrowed and remains the caller's accounting responsibility. Managed
output reservation denial, CRC corruption, cancellation before admission and
between real-source chunks, shared last-owner retention/release are checked.
These authored packages are not Capture One-produced EIP fixtures and do not
qualify adjustment/profile/LCC behavior. EIP remains Pending for viewing.

Current extraction logs: `/tmp/rrrah-eip-deflated-oracle.log` and
`/tmp/rrrah-eip-stored-oracle.log` each record seven passing EIP checks,
including the independent full-source fixture and mid-inflation cancellation.

`decode_eip_sensor` provides explicit sensor-only import from supplied package
bytes, with a required MemoryBudget and shared inflation/output accounting.
It dispatches registered Camera TIFF-family backends directly in memory;
At that checkpoint MRW and unimplemented families returned UnsupportedRaw before
inflation. This API is distinct from authored Capture One appearance, inventories
settings/assets without applying them, and does not add EIP to `decode_image`.
No temporary file or source-path read is performed. The common camera pipeline
now accepts a source owner and drops it before metadata adaptation, preserving
both normal file-source and in-memory source accounting contracts.

The authored IIQ package imports all 17,065,152 sensor samples byte-exact against
the independent corrected LibRaw oracle, with native backend recipe 20. A test
uses a nonexistent path, proving supplied bytes are the input. Shared output
clones retain budget until last owner; source-only capacity admits inflation
but refuses sensor output and releases storage. Invalid camera data, unsupported
member extension and nonzero image index are refused. Nine EIP checks pass in
`/tmp/rrrah-eip-sensor-final.log`; full ordinary decoder regression passes
596 tests in `/tmp/rrrah-eip-sensor-regression.log`. EIP remains Pending for
normal viewer opening and Capture One settings/profile/LCC qualification.

DNG EIP sensor import now uses the shared NativeDngDecoder source-owner pipeline,
with the original full-sensor output admission and strict embedded color/WB rules.
The DNG source remains alive through metadata adaptation, then is released before
return. Native DNG recipe is preserved. Authored ZIP packages around CC0 Pentax
K-x object 830 have reproducible stored/DEFLATE hashes in
`tests/fixtures/eip-dng-package-corpus.json`. All full sensor samples match the
independent LibRaw oracle; CFA, source DefaultCrop [10,10,4288,2848], black grid
[1,9,9,0], white 4094 and WB are checked. LibRaw's different crop is not used
as a replacement for DNG DefaultCrop. Ten EIP checks pass per packaging in
`/tmp/rrrah-eip-dng-final.log` and `/tmp/rrrah-eip-dng-stored-final.log`.
At that checkpoint MRW in-memory import was open. Capture One-authored settings/profile/LCC behavior
remains open; normal EIP viewer routing is still Pending.

CR3 sensor-only EIP import now reuses the native CR3 source-owner pipeline,
including streamed plane workers, CTMD per-image WB/levels and managed sensor
output admission. The compressed source owner is released after workers finish,
before metadata adaptation. Two existing local EOS R8 fixtures are independently
packaged in Stored/DEFLATE ZIPs. All four imports match every independent sensor
sample and native CTMD WB, black 512, white 16383 and crop [168,108,6000,4000].
A nonexistent source path confirms byte-only import. Source-only memory capacity
refuses output without leaking; shared output keeps its reservation until the
last owner drops. Package hashes/rights are in
`tests/fixtures/eip-local-cr3-package-corpus.json`; large source/package files stay
external. Generator selection of a local case requires `--include-local`, and
redistribution rights are not asserted. These are authored ZIP transport fixtures,
not Capture One-created EIP adjustment/profile oracles.

Explicit EIP sensor storage/GPU qualification: eight authored packages (IIQ,
DNG and both existing local CR3 sources, each Stored and DEFLATE) pass
`eip_sensor_readback` on Metal Apple M4 Max. Full pixels, complete metadata,
CPU thumbnails and offscreen SDR frames at 128x96 equal ordinary original RAW
decode, which is separately qualified against independent sensor/color oracles.
RAM leases share storage and refuse eviction during use; persistent disk and
swap preserve pixels/metadata/frame exactly. Full managed-memory pressure
refuses swap restore without losing the entry, then retry succeeds and releases
output on last-owner drop. Cache keys use the outer package's standard sampled
fingerprint, not the inner RAW fingerprint. No Capture One adjustment appearance,
physical HDR presentation or CUDA/NVIDIA execution is inferred from this test.
`/tmp/rrrah-eip-cache-metal.log` records all eight package results.
EIP remains Pending for normal viewer opening and Capture One assets/settings.

MRW sensor-only EIP import now shares the native MRW source-owner pipeline.
Both pinned CC0 Minolta sources (1826 and 4419), each Stored and DEFLATE,
match every independent LibRaw sensor sample and the pinned levels/WB/crop.
Source-only admission refuses the output allocation and releases reservations;
shared output retains its reservation until the last owner drops. The ordinary
decoder suite passes 596 tests after this extraction (34 ignored). Package
hashes are recorded in `tests/fixtures/eip-mrw-package-corpus.json`.
Capture One appearance and ordinary EIP viewer routing remain Pending.

EIP MRW storage follow-up: the explicit sensor test now covers 12 packages
from six RAW sources, including Minolta 1826 and 4419. Actual Metal Apple M4 Max
128x96 frames, full pixels/metadata, CPU thumbnails, RAM leases, persistent
disk and swap restoration remain exact. Memory-pressure refusal preserves the
swap entry for retry and all managed output credits release after the last owner.
Evidence: `/tmp/rrrah-eip-mrw-metal.log`. This does not qualify Capture One
adjustments, ordinary EIP viewer opening, physical HDR or CUDA/NVIDIA.

## MNG-VLC opaque full-canvas foundation

The native MNG parser follows the official VLC chunk/profile contract:
https://www.libpng.org/pub/mng/spec/mng-vlc.html. It checks chunk bounds/types
and CRC throughout the stream with cancellation between CRC blocks, limits
65,536 chunks and 4,096 embedded frames, admits bounded canvas geometry and
requires MEND at exact EOF. Only full-canvas opaque PNG layers in the supported
VLC profile are accepted. Other chunks/profiles and tRNS/alpha are refused explicitly. At the initial
checkpoint global inherited properties were also refused; color inheritance
for cHRM/gAMA/iCCP/sRGB is now supported. At the initial checkpoint multi-layer still
composition was also refused; the opaque full-canvas case is now supported.
Selected images preserve PNG integer precision and color through the existing
bounded PNG backend; no earlier entropy streams are decoded. Reconstructed
PNG source bytes reserve managed memory before copying; output retains its
credit until the last owner drops. Metadata/codec scratch remain separately
bounded and are not total RSS accounting. `decode_mng` exposes tick rate;
common raster/gallery/CLI routing exposes image index/count, including strong
MNG content under a misleading RAW suffix.
`generate-mng-fixtures.py` reproduces tiny CC0 Python/zlib fixtures and expected
integer planes. FFmpeg exactly matches all three extracted PNG planes (RGB8
and Gray16); this does not independently qualify outer MNG composition/timing.
See `tests/fixtures/mng/{manifest,png-oracle-qualification}.json`. Decoder tests:
599 passed, 34 ignored, `/tmp/rrrah-mng-regression.log`. Full MNG semantics,
authentic external MNG oracle, cache/swap/GPU and live viewer proof remain open.

MNG cache/Metal follow-up: all three authored frames (RGB8 animation indices
0/1 and Gray16 still) match pinned FFmpeg PNG sample planes and their prepared
linear-sRGB references. The prepared RAM cache shares pixels through a lease;
zero-limit eviction refuses while pinned and succeeds after release. Native
and prepared representations retain exact samples, color and selection through
streamed swap, pressure refusal and successful retry. All six variant/frame
checks produce identical 96x64 Metal M4 Max frames; managed credits return to
zero after owner release. Evidence: `/tmp/rrrah-mng-cache-metal.log`.
`qualify-mng-png-fixtures.py` reproduces the FFmpeg planes and recorded hashes.
This does not qualify outer MNG controls/composition/transparency, physical
presentation, or photographic color independently of the shared display path.

MNG zero-tick still semantics now match the VLC frame definition: the entire
datastream describes one frame. With admitted full-canvas opaque layers, the
last layer completely replaces previous ones; the decoder selects it directly,
retains PNG precision/color, reports image_count=1 and refuses image_index=1.
A new authored two-layer RGB8 still with distinct layer values exercises this
case; both embedded PNG planes match FFmpeg independently. The decoder suite
passes 600 tests (34 ignored) in `/tmp/rrrah-mng-still-regression.log`. Partial
coverage, alpha composition, background/control chunks, global properties and
full MNG qualification remain open.

The four visible authored outputs, including the two-layer still, also pass
prepared RAM and native/prepared swap/Metal qualification (eight combinations):
`/tmp/rrrah-mng-still-cache-metal.log`. Static count/index and final-layer values
survive restoration, pressure retry and owner release.

MNG global color defaults now snapshot at each IHDR and are inserted into the
selected standalone PNG only when the layer lacks its own color declaration.
Empty top-level color chunks clear their corresponding defaults for subsequent
layers; global sRGB nullifies older gAMA/cHRM, and either gAMA/cHRM nullifies
older sRGB. Inherited iCCP uses existing PNG profile validation/color preparation
and retains managed metadata credit through the last owner. Empty embedded
color chunks and invalid global sRGB/gAMA/cHRM lengths/values are refused.
The default-reset path follows the current PNG policy (explicit AssumedSrgb
when there is no remaining color declaration); unqualified gAMA/cHRM color
interpretation remains Unspecified. No universal gamma/chromaticity rendering
qualification is claimed by this pass-through metadata implementation.
Six decoded visible frames (12 native/prepared variants) pass managed RAM/swap
and exact Metal M4 Max 96x64 readback, including the new two-frame global-sRGB
fixture. Seven embedded PNG sample planes match FFmpeg. Logs:
`/tmp/rrrah-mng-global-regression.log` (602 tests passed, 34 ignored),
`/tmp/rrrah-mng-global-cache-metal.log`. Background, global palette/sBIT/pHYs,
transparent/partial composition, TERM and other controls remain pending.

MNG opaque partial-layer composition now preserves uncovered pixels from the
nearest preceding full-canvas layer. Layers remain at the VLC origin (0,0).
Zero-tick streams return one final composition; animated selection retains
previous pixels outside each new rectangle. Native RGBA8/RGBA16 integers are
copied without display conversion; mixed 8/16-bit layers promote 8-bit values
by 257 into a 16-bit canvas. Different or unqualified color spaces refuse
composition pending a qualified linear conversion path. Initial partial images
without a covering base still require background support and are refused.
The canvas reserves its entire footprint before fallible allocation; each layer
uses the existing admitted PNG pipeline, with cancellation during row copies.
Fixtures add horizontal/vertical partial RGB8 layers, a two-layer static image
and RGB16 values. All 15 embedded PNG planes match FFmpeg; a Python integer
rectangle reference supplies 13 full presentations. 604 decoder tests pass
(34 ignored), including pre-canvas pressure and mixed precision, in
`/tmp/rrrah-mng-partial-regression.log`. All 13 outputs in native/prepared forms
(26 combinations) pass RAM lease, swap/pressure retry and exact 96x64 Metal
M4 Max readback with zero final managed usage:
`/tmp/rrrah-mng-partial-cache-metal.log`. Alpha/background/control semantics,
mixed-profile composition, independent outer MNG engines and physical display
remain unqualified.

Float32 RLA gray/RGB (optional float32 matte) is available through
`decode_rla_float_with_interpretation(request, RlaFloatByteOrder, RlaAlphaMode,
RasterColorSpace)`. Byte order is explicitly Little/Big; the general route
requires `DecodeRequest::rla_float_byte_order` for float records rather than guessing producer byte order.
Raw float channel lengths must equal active width times four; finite RGB values
including negative and HDR values survive unchanged. Alpha must be finite in
[0,1]; premultiplied interpretation refuses zero-alpha emission and numeric
overflow. Working/output pixels share one managed float allocation. A two-pixel
OpenImageIO-authored little-endian fixture and its independent readback agree
exactly with native HDR samples; an authored byte-swapped variant verifies Big.
Common request routing and display-cache byte-order isolation are implemented. Explicit native float-RLA RAM/swap and linear Metal readback are tested against the producer HDR fixture; CLI byte-order selection is available; CLI color selection (`--rla-color srgb|linear-srgb`) is available; physical HDR presentation remains pending.

### EIP original RAW viewer follow-up (2026-10-07)

The common sensor route now opens the original RAW member of EIP. Earlier
pending viewer statements above describe the previous implementation state.
Capture One appearance remains unsupported. Authored real EOS R8 packages pass
common decoding, actual forward/backward app prefetch and disk-cache restoration.
RAM TTL and file swap preserve every sensor value and an actual Metal frame
against the original CR3, including restore refusal and recovery under full
managed-memory pressure. Source identity uses the existing bounded sampled file
fingerprint (size, mtime and sampled BLAKE3), not a full-file cryptographic hash.
This is an implemented original-sensor subset, not full EIP qualification.

### PDF-compatible AI precise fill follow-up (2026-10-07)

Current strict Poppler comparison after bounded 1/256-pixel fill subdivision:
`one.ai` page zero has 2,522 differing pixels (previously 2,790), maximum
channel error 58 (previously 115), and mean absolute channel error 0.033139
(previously 0.098601). Page one remains exact. `VectorApple.ai` still has
33,167 differing pixels and maximum error 214; mean error slightly increases
from 4.004646 to 4.011151. Strict qualification still fails, so AI remains
Pending. Independent analytic and source-curve area oracles, rather than
Poppler equality alone, establish the selected fill-curve precision fix.

Current warm in-process CPU decode plus display preparation, 60 samples after
three warmups: median 598.16 ms for VectorApple, 8.44 ms for one page zero,
and 0.28 ms for one page one. These are CPU timings with no decoded cache;
GPU presentation latency and speed regression are not established. VectorApple
requires further performance investigation. Reports:
`research/ai-precise-fill-poppler-2026-10-07.json` and
`research/ai-precise-fill-cpu-timing-2026-10-07.json`.

Temporary precise-fill storage now participates in the shared managed budget;
exact and one-byte-short admission tests cover release and typed refusal.
Subdivision checks the request generation during both passes. The decoder
library passes 971 tests with 66 ignored; this does not establish complete
parser/renderer memory accounting or a wall-clock cancellation latency bound.


### GPR normal-viewer status correction (2026-10-08)

GPR has an implemented viewer subset, so it is no longer counted among rows
pending implementation. This does not qualify all cameras, storage variants or
full demosaic/color fidelity. The six official camera color oracles, HERO9
quality/cancellation and Fusion warp regression passed in
`research/metal-current-expanded-2026-10-08.json`. The two original HERO9
application prefetch tests were also explicitly rerun: RAM/disk hits, stale
generation cancellation, previous/next windows, visible-image retention and
count/byte bounds passed (2 tests, no skips). They are now included in the
integrated qualification runner whenever `--gpr-corpus` is provided.

Remaining rows pending normal viewer implementation are PTX, MEF, RWZ, BAY,
CAP and AI. Their explicit limitations in the table still apply. The 94
implemented subsets are not 94 fully qualified formats.

### Metal cache, compute and HDR evidence (2026-10-11)

The integrated local Metal run completed 114 passing test executions in nine
suites with no failures. Explicit follow-ups executed all eight hardware/corpus
tests ignored by the ordinary GPU suite; seven add executions to the integrated
run, while the large-upload cancellation test was already executed separately.
The combined evidence is 121 passing executions, including full-sensor compute,
HERO9/Fusion/SD10 display transport and F828 compute. See
[the integrated report](research/metal-integrated-cache-prefetch-hdr-2026-10-11.json)
and [the manual follow-ups](research/metal-current-manual-followups-2026-10-11.json).

Additional checks connect compute output to RAM leases, disk swap and complete
4000x3000 Metal readback, comparing every retained float component with CPU
exposure results. Viewer EXR/HDR TTL restoration bypasses a blocked decoder gate;
actual RAW background-worker reversals preserve both original sensor samples
and complete 6188x4120 frames. These are bounded fixture checks, with detailed
scope and admission limits in the
[compute/cache report](research/metal-large-compute-cache-swap-2026-10-11.json),
[viewer TTL/prefetch report](research/metal-viewer-ttl-prefetch-2026-10-11.json) and
[RAW worker report](research/metal-raw-worker-navigation-2026-10-11.json).

A dedicated native Metal window presented 301 RGBA16Float/ExtendedSrgbLinear
frames over five seconds without validation errors. macOS reported EDR headroom
rising from 1 to 16 after approximately 2.5 seconds. This establishes surface
presentation and OS HDR activation; it does not measure panel luminance or
independent displayed color accuracy. See the
[live HDR surface report](research/metal-live-hdr-surface-2026-10-11.json) and the
[12 MP float-attachment/swap report](research/metal-large-float-hdr-2026-10-11.json).

The eight-frame authored 12 MP collection soak passed 3674 restore/render cycles
over 3601.158 seconds, with exact payload and Metal frame comparisons and zero
final managed root/queue usage. See the
[hour-long collection report](research/metal-long-lived-collection-hour-2026-10-11.json). Diverse large RAW collection playback, physical
slow-disk qualification and complete per-format viewer/color/variant evidence
remain open. NVIDIA hardware checks are deferred at the user's request. These
Metal results do not promote any of the 100 format rows to fully qualified.

After the soak ended, two EOS R8 RAW fixtures measured decode-to-completed-frame
p50 of 139.593/140.895 ms and swap-to-completed-frame p50 of 217.970/171.918 ms
on Metal. Swap was slower in this run. These are warm-filesystem, offscreen
1920x1080 measurements, not window presentation latency or a universal RAW
performance claim. See the
[post-soak timing report](research/metal-raw-timing-after-hour-2026-10-11.json).

A subsequent decode/swap/swap/decode batch comparison passed all payload and
managed-budget checks, but observed concurrent compilation, VM and browser load
caused large timing drift. It does not establish a stable swap speedup or
regression. See the [ABBA timing report](research/metal-raw-abba-timing-2026-10-11.json).

A separate real-camera collection check passed ten forward/backward rounds over
nine CR2/CR3/NEF/ARW/ORF/PEF/RW2/RAF files in one retained cache and swap worker.
All 90 restores preserved sensor samples, metadata and complete 1920x1080 Metal
frames; visible-pin policy refusal, pressure recovery and final zero managed
usage were checked. This is a bounded synchronous nine-file test, not hour-long
large-corpus viewer playback. See the
[real RAW collection report](research/metal-real-raw-collection-2026-10-11.json).

The same nine-file real RAW collection passed a continuous 3600.270-second run:
1477 complete forward/backward rounds and 13293 exact sensor/metadata/Metal-frame
restores, with nine disk writes and zero final managed root/queue usage.
Pressure refusal, cancellation and visible pin checks run on every restore.
This closes the one-hour bounded real-RAW cache traversal check; large diverse
corpora, asynchronous viewer playback and physical slow disks remain open. See
the [hour-long real RAW report](research/metal-real-raw-collection-hour-2026-10-11.json).

The current ordinary Metal GPU suites and the nine-file RAW collection also
passed together through the shared qualification runner: 107 passing test
executions, zero failures, 8 manual tests left ignored in the ordinary GPU
suite. Those ignored cases are not counted as passing here. See the
[integrated real-collection report](research/metal-real-collection-integrated-2026-10-11.json).

The actual RAW background worker also passed nine camera fixtures across eight
extensions with alternating forward/backward plans, a retained one-entry disk
cache, exact original sensor samples/metadata and full-sensor-sized Metal output.
Each plan offers one available RAW neighbor; broader multi-neighbor scheduling
is not established by this test. See the
[mixed-camera worker report](research/metal-mixed-camera-worker-prefetch-2026-10-11.json).

A further actual-worker test offered three real RAW neighbors at once with
behind=1/ahead=2 and verified both forward and backward plan order. All six
restores matched original sensor samples, metadata and 1920x1080 Metal frames;
the selected input stayed absent and disk count stayed three. This verifies
multi-neighbor plans processed by one worker, not parallel decode or mid-flight
reversal. See the
[multi-neighbor RAW worker report](research/metal-multi-neighbor-raw-worker-2026-10-11.json).

The actual filmstrip renderer also passed an RGBA16Float thumbnail check:
sRGB byte 128 samples into the independent expected linear-light value with
exact alpha and released texture/upload credits. This checks one opaque
thumbnail interior pixel, not the full viewer UI or physical panel. See the
[HDR filmstrip report](research/metal-hdr-filmstrip-2026-10-11.json).

The main viewer was visually observed on its native Metal HDR surface with two
RAW files and left/right cache navigation. A separate startup observation showed
the first RAW frame without input after native surface occlusion cleared;
initial blank capture was temporary, with first present logged at 4093.577 ms.
These UI observations do not measure physical HDR color accuracy. See the
[viewer navigation report](research/metal-main-viewer-hdr-navigation-2026-10-11.json)
and [startup report](research/metal-main-viewer-hdr-startup-2026-10-11.json).

Metal qualification found a foreground responsiveness issue: the loader read
`SourceStamp` synchronously before enqueueing every decode request. A native
viewer sample caught the main thread in `SourceFingerprint::from_path` and
`File::open` for a Documents input. The same viewer ran with input copies in
the temporary directory; the cause of the blocked open is unconfirmed. Moving
this validation off the UI thread must preserve source-change detection and
pending/active request coalescing. New selections now enqueue without reading
their source; the decoder worker establishes the active fingerprint. Duplicate
requests for the same in-flight selection now validate on a bounded background
worker. Unchanged inputs retain the active generation; changed/unreadable
inputs schedule one replacement through event-loop polling, including when
the original decode has already finished. Navigating elsewhere discards the
old validation result. Deferred thumbnails submit their source checks to
a single background validator with one active and one queued request. The UI
polls without waiting and keeps the original two-second retry expiry; replacing
or dropping a request cancels its result and retains no pixels in the worker.
Folder-refresh thumbnail cache invalidation now checks a bounded cache snapshot
on the folder-scan worker. The UI applies only evidence matching the cached
texture ID, source path and fingerprint, preserving entries replaced while
the scan was running. A newly selected cover also invalidates the old cover
even if the old file remains unchanged.
This version passed 180 ordinary viewer tests with zero failures and 32
explicit ignored cases, plus a separate actual Metal thumbnail callback test.
The new regressions check blocked-reader responsiveness/cancellation, delayed
evidence preserving a replaced texture and a changed first cover. See the
[background folder validation report](research/metal-background-folder-source-validation-2026-10-11.json).
These changes remove the identified synchronous fingerprint paths, but do not
make blocked kernel reads cancellable or qualify physical slow storage.
Dropped-path metadata inspection now runs on a separate bounded worker with
one active and one queued path. A newer drop replaces queued work; later
gallery/folder navigation cancels a pending drop. The UI polls results and
retains earlier folder-scan context until a drop is accepted or rejected, so
an invalid drop cannot lose the earlier scan. Symlink, unsupported and special
file refusals retain their previous inspection policy. Interactive CLI startup
now admits its initial source on the foreground decoder worker, as described below.
This version passed 189 ordinary viewer tests with zero failures and 32
explicit ignored cases. Six new regressions include real metadata/refusal
cases, blocked-worker/latest-drop behavior, cancellation and actual App state
transitions preserving prior folder context. A separate actual Metal test
released the thumbnail upload budget while drop inspection remained blocked;
this uses a synchronized test reader, not physical slow storage or an OS drag
gesture. See the [asynchronous drop report](research/metal-async-drop-inspection-2026-10-11.json).

The final asynchronous foreground-duplicate implementation passed 183 ordinary
viewer tests with zero failures and 32 explicit ignored cases. Regressions
include a blocked validator, navigation cancelling old evidence, completion
of the original decode before reload, preserving index/window/development and
replacing a successful validation completed before a newer file-change
request. See the [duplicate validation report](research/metal-async-foreground-duplicate-2026-10-11.json).
The repeated integrated Metal run passed 110 tests with zero failures and
eight explicit ignored cases; see the
[integrated report](research/metal-async-fingerprint-integrated-2026-10-11.json).
The final native binary was also observed displaying its first RAW without
navigation and both left/right RAW transitions on its actual HDR surface;
this ordinary navigation observation does not exercise the duplicate-change
race or qualify physical HDR color. See the
[final native viewer report](research/metal-main-viewer-async-fingerprint-final-2026-10-11.json).

The deferred-thumbnail change passed all 177 ordinary viewer tests with zero
failures and 32 explicit ignored cases, followed by a separate passing actual
Metal callback test. Focused checks cover changed/missing inputs, a blocked
reader, queue replacement and expiry releasing managed pixels while validation
is still blocked. These focused tests are included in the ordinary count.
See the [asynchronous thumbnail validation report](research/metal-async-thumbnail-validation-2026-10-11.json).

The shared qualification runner's `--metal-mixed-raw-workers` option includes
both real mixed-camera and three-neighbor worker checks, requires a RAW
manifest with at least five distinct existing files, and rejects passing
harness output without the expected per-source and completed-plan evidence.
The integrated run passed 110 tests with zero failures and eight explicit
ignored tests; both worker suites supplied their required workload evidence.
See the [integrated report](research/metal-mixed-raw-workers-integrated-2026-10-11.json)
and [synthetic runner controls](research/metal-mixed-raw-worker-runner-controls-2026-10-11.json).
The foreground source-validation change separately passed 62 tests with zero
failures and nine explicit ignored tests; see its
[regression report](research/metal-foreground-source-validation-2026-10-11.json).


Interactive startup admission now checks the required source fingerprint on the
foreground worker, removing the pre-window `is_file()` call. Cancellation is
checked before and after admission; obsolete failures are discarded. External
WAL palette identity and regular-file symlink behavior are preserved. Invalid
interactive startup paths now open the viewer and show a load failure instead
of exiting before window creation; headless `--inspect` remains synchronous.
Five added regressions cover cancellation, blocked admission, non-files,
palette identity and recovery after a failed startup. The final viewer suite
passed 194 tests, zero failures, with 32 explicit ignored cases.
The rebuilt native viewer rejected a real FIFO without hanging and displayed
a CR3 on Apple M4 Max Metal with an Rgba16Float ExtendedSrgbLinear surface;
the upload consumer lease returned to zero. Initial occlusion delayed the first
visible frame. This is neither a physical HDR color/luminance qualification nor
a physical slow-disk test. See the [startup admission report](research/metal-async-startup-admission-2026-10-11.json).


A combined current-source Metal regression after asynchronous startup admission
completed all 12 requested suites: 119 tests passed, zero failures, eight
explicit ignored cases. This includes 20 live count/size/TTL policy cycles,
nine real RAW sources through ten collection rounds (90 restores), actual
mixed-camera and three-neighbor workers, mixed-gallery reversals, RAW worker
reversal, 12 MP float HDR swap/readback, 128 viewer HDR TTL restores without
decoder fallback plus two preloads under a blocked decoder, large upload
cancellation and compute/cache/swap. All explicitly requested suites recorded
passing execution on Apple M4 Max Metal. Source files were not edited during
this run. See the [combined current-source report](research/metal-current-integrated-2026-10-11.json),
which includes log hashes and source hashes. This run does not replace physical
HDR/slow-disk/OS-wide pressure qualification, broaden the nine-source RAW
corpus to all 100 formats, or qualify the eight ignored cases or NVIDIA.


A cross-format transport regression now checks 43 fixture sources in both
native decoded/windowed and display-prepared storage: 86 cases on Apple M4 Max
Metal. It checks visible-lease pinning, spill after unpinning, cancellation after
restore admission, refusal under full managed pressure, exact sample bits,
color/profile, dimensions and image selection, identical 64x64 Metal readback,
and final zero managed/queued bytes with one successful write/read per case.
FITS/NRRD/MRC/DICOM use an explicit [-100, 1000] display window; RLA uses
explicit straight alpha and sRGB. Other untagged inputs explicitly request
sRGB. This exposed and fixed ignored explicit sRGB assumptions in untagged
PSD/PSB and JPEG2000 codestreams. Strict unspecified-color refusal remains;
embedded PSD ICC and JP2 declared color retain precedence. Decoder tests after
the fixes passed 985 cases (67 ignored); viewer tests passed 194 (32 ignored).
Use `--metal-raster-collection` with the qualification runner; its evidence
check rejects missing/duplicate cases and incomplete or nonzero-budget output.
See the [raster collection report](research/metal-raster-collection-2026-10-11.json).
The collection proves bounded cache/swap/display transport, not independent
pixel/color decoding for every format, all variants, physical HDR, physical
slow-disk/OS-wide pressure, or completion of the 100-row scope.


The new raster collection was subsequently executed end to end through the
qualification runner with `--gpu-backend metal --gpu-only --metal-raster-collection`.
Both requested suites completed and passed evidence validation: 108 passing test
executions, zero failures, eight explicitly ignored cases in the ordinary GPU
suite. The collection itself executed all 43 sources in both storage modes
(86 cases); the verifier accepted the complete unique-case inventory and zero
final ownership. Source/log hashes and exact commands are in the
[runner integration report](research/metal-raster-runner-2026-10-11.json).
This verifies the repeatable runner integration after the PSD/PSB/J2K fixes;
it retains all corpus, color-oracle, physical HDR and slow-storage limitations
of the preceding collection report.


The raster collection now additionally compares every binary16 RGBA component
of a 64x64 linear HDR `Rgba16Float` attachment before/after swap for all 86
storage cases. A shared-device HDR readback helper releases renderer credit
after each completed submission. An independent constant-color control checks
every pixel of a padded 63x65 target: linear [4,-1,0.5,1] at +1 stop must produce
binary16 [0x4800,0xc000,0x3c00,0x3c00] ([8,-2,1,1]), preserving above-one and
negative values and excluding row padding. Actual Apple M4 Max Metal execution
passed all 86 SDR/HDR cases and the independent control. The runner verifier
now requires matching HDR cases and this control; missing/duplicate HDR cases,
a missing control or the previous SDR-only log are rejected. See the
[HDR collection report](research/metal-raster-collection-hdr-2026-10-11.json).
This extends offscreen transport proof; it does not qualify physical HDR display
color/luminance, full-resolution targets for all sources or independent decoding
of all 100 formats.


Two explicit empirical Metal HDR range-edge tests also passed. Every pixel of
a 64x64 attachment saturated [+65504,-65504,0.5,1] at +10 stops to binary16
[0x7bff,0xfbff,0x6000,0x3c00]. A 63x65 attachment interpolating opaque
[+f32::MAX,+f32::MAX,0.5,1] and [-f32::MAX,+f32::MAX,0.5,1] produced a zero-red
center and finite components throughout. These tests intentionally remain
manual/ignored in ordinary suites: they observe this Metal backend beyond the
renderer's documented portable binary16 caller range and do not establish a
cross-backend saturation contract. Production shader code was unchanged.
See the [Metal range-edge report](research/metal-hdr-range-edges-2026-10-11.json).


The raster transport collection has expanded from 43 to 62 sources (124
native/windowed and display-prepared storage cases). Added sources cover CUR,
JNG, JPEG-LS, profiled JPEG-XR, Farbfeld, Basis, XBM/XPM, DCX, SGI, SUN,
ILBM/PBM, PICT, MacPaint, Softimage PIC, Alias PIX, PFM and WAL. All 124 cases
passed exact sample/color/selection and SDR plus binary16 HDR frame checks
through RAM/spill/cancellation/managed pressure/swap on required Metal.
The new original CC0 PFM fixture also matches its authored top-first binary32
oracle, preserving bottom-first storage, negative/HDR values and adjacent
float levels under an explicit linear-sRGB assumption. WAL discovers the
external PCX palette through a temporary canonical textures/pics tree and
matches the existing Pillow palette oracle. The runner now requires all 124
unique SDR/HDR cases and rejects the older 43-source evidence as incomplete.
See the [expanded collection report](research/metal-raster-collection-expanded-2026-10-11.json).
Counts describe fixture sources, not fully qualified format rows. Remaining
format variants, independent color/decoder oracles, physical HDR and physical
slow-storage/OS-wide pressure limitations still apply.


Raster collection coverage now includes the first and last advertised image
selection per source: 62 sources, 77 selected images and 154 native/windowed or
prepared storage cases, all passing on required Metal. Multi-selection cases
include GIF, WebP, APNG, Aseprite, DCX, STI, WAD, DDS/KTX/KTX2/PVR mips and
FITS/NRRD/MRC/DICOM sections/frames. The DDS/KTX/KTX2 fixtures were changed to
multi-level sources, and both selected mip endpoints match their existing
native pixel oracles. Swap checks now explicitly retain hotspot, pixel aspect
and sample scale alongside image selection, color/profile and dimensions.
Every case still verifies sample bits, SDR and binary16 HDR readback,
cancellation/managed pressure and zero final ownership. The runner requires
all 77 unique selections in both storage modes and rejects an all-index-zero
replacement or the earlier collection log. See the
[selected-image collection report](research/metal-raster-collection-selection-2026-10-11.json).
This covers endpoint selections, not all intermediate frames/mips or independent
color/decoder qualification for all 100 formats; physical HDR and physical
slow-storage/OS-wide pressure remain separate open requirements.


The collection now executes every advertised index in the fixed corpus rather
than just endpoints: 62 sources, 91 selected images, 182 native/windowed and
prepared storage cases. All passed on required Metal with the same pixel,
metadata, SDR/HDR, swap/cancellation/managed-pressure and ownership checks.
Every selected DDS/KTX/KTX2 mip also matches its existing native pixel oracle.
For each of the 62 sources, requesting index=image_count failed while retaining
the original raster and leaving managed used bytes unchanged; this includes
index=1 for single-image sources. The runner verifier requires a contiguous
0..image_count index inventory for each source in both storage modes, matching
HDR proofs and a bounds-refusal record. Missing/duplicate cases, absent bounds,
an incorrect advertised range, all-index-zero replacements and the previous
endpoint-only report are rejected. See the
[all-index collection report](research/metal-raster-collection-all-indices-2026-10-11.json).
This closes intermediate-selection coverage for these fixtures only. It does
not close all variants, independent color/decoder qualification for all 100
formats, physical HDR or physical slow-storage/OS-wide memory pressure.


The `raw_view_timing` example now supports `--raster-view --swap-view SOURCE`.
It seeds a prepared raster outside timing, releases its managed RAM, restores
it 18 times (three warmups, 15 measured), checks every sample bit and metadata
outside the timed intervals, reports separate restore/upload/completed-frame
quantiles, and requires one write, 18 reads, zero errors/queued bytes and final
CPU/GPU credit zero. Exposure-compute and raster-swap timing remain separate
modes so restore timings cannot silently include development.
An ABBA run on an authored 4000x3000 sRGB PNG (2,890,441 compressed bytes,
192,000,000 prepared bytes) produced decode+prepare+upload+completed-frame p50
383.840 then 197.374 ms, versus swap totals 333.555 and 325.565 ms. Restore alone
was 248.998 and 245.464 ms. These results do not establish a general swap speedup;
they identify prepared-raster restore as a concrete profiling target and show
substantial warmup/system-load sensitivity in decode. See the
[raster swap timing report](research/metal-raster-swap-timing-2026-10-11.json).
This is a warm local-disk offscreen SDR timing, not real display latency, physical
HDR, slow-disk qualification or a broad format/camera performance claim.


A bulk direct-read restore experiment was evaluated and rejected as an unproven
speed optimization. It bypassed the swap staging buffer for large empty-buffer
reads and increased raster codec read portions to 256 KiB, keeping the store's
64 KiB I/O/cancellation bound, hashing and object format unchanged. Experimental
validation passed 128 cache and 11 swap tests plus all 182 Metal collection
cases. However, ABBA baseline restore p50 was 240.694/240.125 ms versus
241.079/236.704 ms for the experiment; whole-path timing also overlapped.
Both production edits were reverted. A regression for mixed short header,
bulk read and short tail with exact bytes/hash was retained. See the
[direct-read experiment report](research/metal-raster-swap-direct-read-experiment-2026-10-11.json).
These dev-profile measurements do not justify a production speedup claim;
release-profile timing is required before further performance decisions.


Release-profile ABBA on the same authored 12 MP PNG confirmed that prepared
192 MB swap is slower than decode+prepare for this fixture: whole-path p50
346.324/326.036 ms versus 210.211/141.966 ms; swap restore alone
251.874/235.408 ms. All 36 swap restores preserved sample bits and metadata,
with zero errors, queued bytes and final managed CPU/GPU credits. These are
completed offscreen SDR Metal frames, not physical display latency or HDR
color qualification. See the [release timing report](research/metal-raster-swap-release-timing-2026-10-11.json).

The separate release `rrrah-cache` example `raster_restore_profile` uses an
original finite 4000×3000 RGBA32F payload, 3 warmups and 15 measured restores.
Total p50 was 218.895 ms, timed stream read/hash 216.144 ms, and non-stream
remainder 2.628 ms. Separate zero-init and BLAKE3 controls were 1.766 ms and
189.885 ms. Controls are independent operations, not exact additive stage
attribution. All 18 restores matched source values and metadata, ending at
zero managed bytes. This points to integrity hashing as the next profiling
target without weakening integrity or cancellation. Warm local disk and one
synthetic payload do not qualify physical slow disks, OS-wide pressure or
all supported formats. See the [stage profile](research/raster-restore-stage-profile-2026-10-11.json).


A follow-up release BLAKE3 control confirms the Apple Silicon build already
uses NEON. Serial 64 KiB, 1 MiB and whole-input update boundaries showed little
difference (about 188–192 ms for 192 MB). The optional
`rrrah-cache/restore-profile-parallel` profiling feature uses dedicated pools:
2 threads / 4 MiB portions measured p50 102.007 ms; 4 threads / 4 MiB measured
73.635 ms. All 72 parallel digests matched serial hashing. This is a hash-only
experiment, not a production swap acceleration: bounded scratch admission,
cancellation, concurrent restores and whole-path Metal timing still need
integration and qualification. No production integrity/cancellation path was
changed. See the [parallel hash controls](research/raster-swap-hash-parallel-controls-2026-10-11.json).


Parallel integrity hashing is now integrated for native raster swap restores.
One lazy process-wide pool (up to four threads) shares one 4 MiB scratch slot;
scratch is charged to the supplied restore admission root, independent of the
pixel-only cap. Conservative headroom admission, unavailable slot/pool or
allocation pressure select serial restore. Custom/expanding payloads and
mosaic payloads remain serial by default. I/O polls cancellation every 64 KiB;
a joined hash batch is at most 4 MiB. Thread stacks (up to four × 512 KiB) and
pool bookkeeping remain outside managed buffer accounting. Payload/digest
formats and exact-length/hash checks are unchanged.

Actual Metal release ABBA on the authored 12 MP PNG measured restore p50
168.399/201.928 ms serial versus 124.344/130.905 ms parallel; whole-path p50
234.881/265.772 ms versus 200.470/209.830 ms. All 72 restores preserved every
sample bit and metadata, ending with zero managed CPU/GPU and queued credits.
The final cache/swap gate passed 156 tests (three manual stress tests ignored),
including exact pixel cap plus large ICC profile, budget replacement, custom
expanding-codec admission, concurrent-store fallback, cancellation and damaged
objects. The actual Metal collection again passed 62 sources / 91 selections /
182 storage cases, exact SDR/HDR frames and all advertised-index bounds.
This bounded warm-local-disk result does not establish physical display latency,
physical HDR color, slow-disk or all-format qualification. See the
[production integration report](research/metal-parallel-raster-swap-2026-10-11.json).


The strengthened large-collection Metal qualification passed a 180.259-second
loop over eight original 4000×3000 linear RGBA32F frames: 359 successful
accelerated restores, 359 cancellations after scratch admission, 359 full-root
pressure refusals and 359 preallocation cancellations. Each successful restore
matched every float-component bit, the full 4000×3000 SDR readback and a 63×65
binary16 HDR readback against pre-spill GPU references. There were only eight
initial writes, no redundant writes, errors or dropped spills; managed root,
queue and RAM membership returned to zero each cycle. Scratch activation was
observed through charged root use above the exact 192 MB pixel allocation.
See the [sustained parallel-swap report](research/metal-parallel-raster-swap-sustained-2026-10-11.json).
This is bounded managed pressure and warm local disk, not physical slow-disk,
OS-wide memory exhaustion, physical HDR output or 100-format qualification.

A read-only AppKit diagnostic is available as
`swift scripts/inspect-metal-display.swift`. The local system reports an online
built-in Liquid Retina XDR display: potential EDR 16× and current EDR 1× at the
snapshot. Current headroom is dynamic; these metadata do not measure physical
luminance/color or prove active viewer HDR presentation. See the
[display inventory](research/metal-display-edr-inventory-2026-10-11.json).


Native-window HDR activation was checked on the built-in Liquid Retina XDR
using an original 1024×512 Radiance control with declared Rec.709 primaries and
eight linear neutral levels from 0.125 to 16. The actual Metal HDR surface was
RGBA16Float / ExtendedSrgbLinear: AppKit current EDR changed from 1× to 16×
while the HDR window was visible, returned to 1× after quitting, and was 1×
with the same control in the SDR BGRA8UnormSrgb / Auto surface. This establishes
active OS EDR admission and visible application presentation, not measured
physical luminance, color accuracy or highlight differentiation above SDR white.
The control is reproducible with `scripts/make-hdr-display-control.py`.

This native check exposed a view-update bug: exposure keys wrote GPU uniforms
without requesting a redraw, leaving the old image until an unrelated event.
The common application view update now requests redraw, also covering zoom,
pan and reset. CUA verified that minus keys alone reveal all eight control
levels at −4 stops in both HDR and SDR, and reset immediately restores the
original appearance. All 194 ordinary viewer tests passed (32 actual-GPU/manual
tests ignored by that gate); native HDR/SDR checks were performed separately.
The test viewer was closed and its prior temporary launcher restored. See the
[native EDR and redraw report](research/metal-native-edr-and-view-redraw-2026-10-11.json).


The normal viewer now accepts `--untagged-color linear-srgb` for linear RGB
whose primaries are unspecified (including PFM), forwarding that explicit
choice to foreground loads, neighbour preload and thumbnails. Strict remains
the default, and encoded-sRGB and linear assumptions have separate cache keys.
The original 2×2 PFM matches its authored binary32 oracle through CLI preload,
TTL spill and decoder-free swap restore; strict/sRGB requests cannot reuse it.
Thumbnail last-owner credit release and exact Metal SDR/HDR pre/post-swap frames
pass. The ordinary viewer gate passed 196 tests (33 manual/GPU cases ignored),
and the new actual Metal case was run separately. CUA also opened this PFM in
the normal HDR window (RGBA16Float / ExtendedSrgbLinear, current EDR 16×).
This closes the bounded viewer-policy gap, not corpus or alternate-PFM-dialect
qualification. See the [PFM linear CLI report](research/metal-pfm-linear-cli-2026-10-11.json).


ASTC HDR now honors explicit `--untagged-color linear-srgb`. The 23×17
compressed HDR fixture preserves every binary32 sample through preload, TTL
spill and decoder-free restore; incompatible strict/sRGB cache requests refuse.
Actual Apple M4 Max Metal SDR and HDR readbacks match exactly before/after swap.
All 986 ordinary decoder and 197 viewer tests pass; actual Metal was run
separately. This bounded reference-family check does not qualify all producers,
linear LDR, 3D slices or physical HDR output. See the
[ASTC HDR report](research/metal-astc-hdr-linear-policy-2026-10-11.json).


KTX2 imports now apply explicit `linear-srgb` only when DFD primaries are
absent (zero). Declared non-sRGB or unknown nonzero primaries retain their
strict unsupported-display behavior; native float sample bits remain unchanged.
Nine KTX2 unit tests and the complete ordinary decoder gate (987 passed,
67 ignored) pass. Viewer preload, TTL spill and decoder-free restore preserve every float bit;
actual M4 Max Metal SDR/HDR frames match exactly before/after swap. The full
viewer gate passes 198 tests (35 ignored); the new Metal case passed separately.
This is not full producer or physical HDR qualification. See the
[KTX2 policy report](research/metal-ktx2-linear-policy-2026-10-11.json).


DPX explicit `linear-srgb` imports now qualify only undefined colorimetry
(255), with declared linear transfer (2) and normalized 0–1 reference range.
User-defined (0) and declared video colorimetry remain strict; non-normalized
reference quantities are not silently reinterpreted. Three DPX unit tests pass,
including unchanged native samples across the new policy. Viewer preload/TTL/swap preserves prepared float bits and rejects incompatible
policy reuse. Actual Apple M4 Max Metal SDR/HDR frames match before/after restore.
The decoder gate passes 988 tests, viewer gate 199; the new GPU case passes
separately. This bounded subset is documented in the
[DPX policy report](research/metal-dpx-linear-policy-2026-10-11.json). Field definitions were
cross-checked against [OpenImageIO libdpx Characteristic](https://github.com/AcademySoftwareFoundation/OpenImageIO/blob/main/src/dpx.imageio/libdpx/DPXHeader.h).


Explicit-layout BAY file import now caps source allocation at the chosen
camera layout before reading. Oversized files refuse with `InputTooLarge`
without allocating managed bytes. All four BAY tests pass, retaining full
8-bit/10-bit storage, padding, cancellation and ownership checks. This closes
a source-admission gap, not camera/color/normal-viewer qualification. See
[BAY source-cap evidence](research/bay-source-admission-cap-2026-10-11.json).


2026-10-11: The actual Metal raster collection now includes compressed ASTC
HDR and plain/DEFLATE RGBA32Float KTX2: 65 fixture sources, 94 advertised
selections and 188 native/prepared storage cases. Full float samples match
their reference/authored oracles; RAM/swap, pressure refusal, cancellation,
SDR/HDR readback and final root/queue release pass. The runner verifier accepts
the expanded log. File counts are not fully qualified format counts, and this
is not physical HDR measurement.
[Expanded collection evidence](research/metal-raster-collection-hdr-expanded-2026-10-11.json).


2026-10-11: Viewer loader/preload ping-pong qualification now includes PFM
and compressed ASTC HDR alongside profiled PNG, GIF, EXR and Radiance HDR,
using the shared explicit linear-sRGB CLI interpretation. All 120 selections
preserve exact Metal SDR/HDR frames, bounded neighbours and RAM membership,
visible-owner reuse under managed pressure, stale-generation refusal and final
root release. The CPU case and actual M4 Max Metal case pass separately;
199 ordinary viewer tests pass. This synchronous helper test does not qualify
asynchronous worker timing or physical HDR.
[Evidence and limits](research/metal-linear-six-format-navigation-2026-10-11.json).


2026-10-11: The actual bounded thumbnail worker now has a linear-import
generation-replacement regression: held ASTC HDR returns no stale thumbnail,
queued KTX2 is replaced without starting, and only the latest PFM publishes
the expected pixels. All managed thumbnail memory releases. The real loader
and queue are exercised with a barrier before decoding; this is not native
ASTC mid-computation cancellation or foreground event-loop qualification.
The focused test and 200 ordinary viewer tests pass.
[Worker evidence](research/linear-thumbnail-worker-reversal-2026-10-11.json).


2026-10-11: CUA opened the compressed 23×17 ASTC HDR fixture in the normal
viewer with explicit linear-sRGB. The native Metal window reported image-ready
in 6.78 ms, RGBA16Float / ExtendedSrgbLinear and upload pending zero; AppKit
reported current EDR 16×. Exposure keys reveal colored gradients immediately,
and reset restores the original appearance. The test app was closed and prior
temporary launcher restored. This single-source observation is not a benchmark
or physical luminance/color qualification.
[Native ASTC evidence](research/metal-native-astc-hdr-window-2026-10-11.json).


2026-10-11: Explicit PFM top-down import passes five decoder tests, both
viewer/cache and actual Metal regressions. The same file has distinct row-order
RAM/swap entries; decoder-free restore preserves rows and exact SDR/HDR frames.
Final memory returns to zero after queued spill completion. Full gates: 990
decoder / 203 viewer tests pass. Adobe-produced interoperability and physical
HDR remain unqualified.
[Evidence](research/metal-pfm-top-down-import-2026-10-11.json).

### 2026-10-11: original pending-RAW fixture search

A current 2016-entry raw.pixls.us repository snapshot contains one original MEF and no CAP/PTX/BAY/RWZ download extensions. This is fixture availability in one repository, not proof that samples do not exist elsewhere and not permission to replace them with renamed IIQ/PEF. The six pending rows retain their acceptance requirements: MEF WB/public import, RWZ original-pixel reconstruction, BAY camera metadata/viewer routing, PTX producer fixture, CAP original structure/fixture, and strict AI fidelity. Evidence: `research/pending-raw-original-fixture-availability-2026-10-11.json` records repository snapshot SHA256 and exact matching download URLs.

### Explicit unqualified MEF color route (2026-10-11)

MEF extension now resolves to explicit unqualified-color route instead of generic DNG fallback; recipe/decode return MEF white-balance/color-support diagnostic. Generic TIFF keeps MEF choice, strong supported container magic still wins. Real pinned ZD public refusal has zero managed allocation peak and no preview; full independent native sensor comparisons remain exact. Eleven MEF tests passed with complete source/oracle environment (initial run omitted oracle variable); full decoder regression1005 passed/78 ignored. Evidence: `research/mef-explicit-color-refusal-2026-10-11.json` and hashed logs/source. Color/viewer acceptance remains pending; sensor-only import unchanged.

### PNG physical pixel aspect to Metal and swap (2026-10-11)

Implemented bounded pHYs parsing before IDAT with CRC/length/duplicate/unit validation; positive densities produce pixel width/height=Y/X, zero density keeps unknown aspect. Ordinary PNG and APNG result metadata retain aspect; PNG EXIF axis-swapping rotations invert it. Decoder regression1006 passed/78 ignored including actual encoded sample preservation, ratios0.5/2, zero density, duplicate/CRC and EXIF90 cases. Explicit authored4x4 PNG Metal test:linear preparation, exact64x64 SDR/HDR against aspect0.5 reference, square-pixel control differs and analytic center/side checks, swap preserves aspect/frames, final source/queue zero, one write/read/no errors. Evidence: `research/png-physical-aspect-2026-10-11.json` and hashed logs/source/fixture. Initial GPU test incorrectly supplied encoded samples to prepared-only helper; corrected test preparation, shader unchanged. APNG propagation lacks a dedicated pHYs selected-frame case; physical display/full PNG acceptance remains open. Source: https://www.w3.org/TR/png-3/#11pHYs .

### Selected APNG physical aspect Metal gate (2026-10-11)

Authored two-frame4x4 APNG (opaque white/green SOURCE, pHYs2/1, repeat3) through common selected-frame decode:indices/count and aspect0.5 retained, both full64x64 SDR/HDR frames exactly match independent authored linear reference rasters. Two swap writes and reverse-order reads preserve metadata/frames, source/queue finalzero/errorszero. New --metal-png-physical-aspect driver option requires both PNG/APNG workload markers and actualMetal adapters; fresh gate133 passed/13 ignored across ordinaryGPU plus selectedPNG/APNG suite, every executed row verified. Evidence: `research/png-apng-physical-aspect-current-2026-10-11.json` and linked hashed logs/report/source/fixture. APNG transient canvas/scratch and direct decode_apng ownership are not yet fully request-budgeted; wrapper retained-pixel admission is insufficient proof of full peak bounds. PhysicalHDR/playback/variants remain open.

2026-10-11 selected16-bit APNG: replaced unconditional RGBA8 intermediate with depth-aware RGBA16/RGBA8 conversion and doubled conversion reservation for16-bit. Earlier16-bit sources were explicitly refused (not silently truncated); now the supported selected-frame path retains low bits and alpha. Two1x1 authored sRGB frames with adjacent16-bit RGB/alpha values match independent f64 transfer-function expectations; output lifetime charged16bytes then zero. Initial regression correctly exposed the old8-bit-only guard; removed that guard for16-bit and also installed codec limits at initial metadata decoder construction. Full decoder1005 passed/71 ignored. Actual Metal suite3 passed, including two16-bit frames against independent linear SDR/HDR references and existing PNG/APNG swap cases; integrated report all executed suites verified. Evidence: research/apng-16bit-precision-2026-10-11.json and retained full logs/report/source/fixture hashes. Full16-bit variants, color profiles, playback and shared accounting of private codec scratch remain unqualified.

2026-10-11 packed APNG: removed8/16-only admission guard; PNG codec validates depth/color legality and expands packed samples. Authored1-bit grayscale,2-bit palette+tRNS(0/64/128/255 alpha),4-bit grayscale sources each have two SOURCE frames in reverse pixel order. Six selected frames match independently calculated f64 sRGB/alpha oracles, indices/count and retained64byte managed canvas; final zero credit. Full decoder1006 passed/71 ignored. Actual Metal selected PNG/APNG suite4 passed, including packed six-frame SDR/HDR comparisons before/after six swap writes/reads, errors0/source+queue0. Integrated ordinaryGPU and manual suite both verified. Evidence: research/apng-packed-depths-2026-10-11.json and retained report/log/hash directory. Full packed variants, playback and private codec shared-budget accounting remain unqualified.

2026-10-11 APNG chunk ownership: track default-image IDAT run, current animation data ownership and completion in CRC-validated scan. Refuse IDAT attached to a later animation frame, fdAT attached to default image, empty controlled frames and partial default-frame geometry before pixel admission. Four valid-CRC structural mutations refuse with zero used/peak shared credit. Positive separate static preview plus split consecutive IDAT reproduces all selected last-frame float samples and animation count4. Full decoder1007 passed/71 ignored; current actual Metal PNG/APNG four manual tests passed, SDR/HDR/swap and packed/16-bit precision markers retained. Specification: https://www.w3.org/TR/png-3/#11fcTL . Evidence: research/apng-chunk-ownership-2026-10-11.json and hashed logs/source. Lazy skipped compressed payloads are not fully inflated/validated; playback/all variants remain unqualified.

2026-10-11 application APNG TTL/cache/prefetch: ordinary and explicit actual-Metal tests use production load_cached_raster_mode/preload_raster helpers with16-bit and2-bit palette APNG sources. Each source stores two distinct selected frames under RAM128bytes/count2/TTL1second, keeps fresh entries, spills both after elapsed TTL, then restores1→0 via real prefetch while foreground decoder gate is held (5-second cancellation watchdog). RAM disabled by count0 for restore isolation; disk count2/65536bytes, queue128/count2, restore128. All serialized payloads/index/count and SDR/HDR Metal frames exact;2writes/2reads per source/errors0 and final shared credit0. Updated --metal-viewer-ttl-prefetch gate now requires new APNG suite as well as existing EXR/HDR scenario:131 ordinaryGPU passed/13 ignored plus1 HDR and1 APNG manual tests passed; all rows verified. Evidence: research/apng-viewer-ttl-prefetch-2026-10-11.json and retained report/log/hash directory. This is bounded real-time TTL and selected-frame integration, not animation playback/long collections/physical slow disk qualification.

2026-10-11 large APNG admission: replace arbitrary four-times-output512MiB dimension rejection with actual RGBA32Float output bound; managed canvas/frame/linear/PREVIOUS reservations govern simultaneous working admission. Authored4000x3000 two-frame16-bit APNG now decodes selected1 under512MiB shared limit. Full and128MiB partial pressure cause clean refusal; retry preserves all48million components within1e-7 of independent f64 sRGB oracle and bit-exact original/restored constant field. Managed decode peak480554140bytes; retained192000000; final CPU/queue0. Actual Metal128x96 SDR/HDR exact against1x1 equivalent-aspect constant reference before/after192MB swap; one write/read/errors0. Full decoder1007 passed/71 ignored; new --metal-large-apng integrated gate131 ordinaryGPU+1 manual passed/13 ignored, all rows verified. Evidence: research/large-apng-memory-metal-2026-10-11.json and retained logs/report/source/fixture hashes. Constant authored content is capacity/numeric evidence, not varied-content offset qualification; PNG private scratch excluded from tracked peak; no physical display claim.

2026-10-11 APNG PREVIOUS patch ownership: save only frame rectangle into managed buffer and restore rows in place, retaining original canvas allocation. Cancellation checked per save/restore row. Independent authored4x3 canvas/2x2 offset patch regression admits192+64bytes (full canvas copy would exceed256-byte limit), proves snapshot offsets, restores every original component including surroundings, finalzero. Full decoder1008 passed/71 ignored with existing composition and pressure oracles. New actual-Metal test selected3→1→2 disposal frames against independent5x3 float composition files; full64x64 SDR/HDR exact, finalzero. Updated PNG/APNG integrated gate131 ordinaryGPU plus5 manual passed/13 ignored, all rows verified. Evidence: research/apng-previous-rectangle-2026-10-11.json and retained report/log/hash directory. Memory savings equal patch-vs-canvas difference; runtime speedup not benchmarked. Full playback/variant/codec scratch accounting still unqualified.

2026-10-11 PREVIOUS paired timing and full-frame fast restore: on12MP canvas/64x64 region,9 alternating measured pairs after3 warmups give prior managed full-copy median38.176250ms versus production rectangle helper0.019000ms. Managed peaks384000000 versus192065536bytes; all48million components match authored position-dependent field bit-exact after both paths; final0. This isolates save/overwrite/restore/release, not full decode/viewer speedup; other host work affects timings. Preserve full-canvas PREVIOUS ownership transfer rather than copying back all samples; new3-frame regression full-white→full-green PREVIOUS→partial-blue OVER proves white base restored and untouched pixels intact, final64byte output then0. Full decoder1009 passed/72 ignored (benchmark separately executed). Current Metal gate131 ordinaryGPU+5 PNG/APNG+1 largeAPNG passed/13 ignored, all rows verified. Evidence: research/apng-previous-paired-current-2026-10-11.json and retained paired/raw logs/report/source hash.

2026-10-11 broad post-policy regression: current ordinary rrrah-decode, rrrah-gpu and rrrah app library/binary/integration suites completed70 harnesses,1418 passed,0 failed,226 explicitly ignored; actualMetal required with optionalGPU disabled. App213 passed/59 ignored. Evidence: research/cross-format-cache-metal-2026-10-11.json and full hashed log. This broad regression covers implemented fixture paths; skipped manual/corpus cases remain unverified and it does not prove full100-format, independent color or physical HDR qualification.

2026-10-11 selected TIFF pages: native bounded top-levelIFD walker supports classicTIFF/BigTIFF both byte orders, checks complete tables/next pointers, caps4096 pages/entries, rejects cycles/missing pages and cancellation before codec. Temporarily redirects firstIFD pointer in already admitted mutable source then restores after success/error; original pixel/tag offsets remain absolute, no whole-file copy. Existing codec applies selected orientation/ICC/alpha path. New3 regressions validate authored gray8 pages/orientation, source preservation, malformed chains, magic file route and failed admission. Full decoder1012 passed/72 ignored. Evidence: research/tiff-selected-pages-2026-10-11.json. Newpage-specific compressed16/float, ICC/alpha and viewerRAM/swap/Metal qualification remain open; TIFF row still not fully qualified.

2026-10-12 selected TIFF viewer metadata/cache/Metal: bounded complete top-levelIFD count now attached with selected index to first and later TIFF/BigTIFF pages; source pointer restored after decode before metadata/color conversion. Full DICOM signature retains precedence over arbitrary TIFF-like preamble, caught by existing DICOM oracle regression; failed/passing logs preserved. Decoder1012 passed/72 ignored. ActualMetal four authored classic/BigTIFF LE/BE gray8 files with second-pageorientation6: explicit assumed-sRGB then linear preparation, independently authored pixel references,63x65 SDR/HDR exact, count1/32byteRAM, full-budget liveRAM hit, TTL0 visible protection then spill, reverse1->0->1 restore with selection metadata, writes2/reads3/errors0 each; final CPU/queue0. Evidence: research/tiff-pages-cache-metal-2026-10-12.json. Initial test GPU refusal without color preparation and expired-lease refusal preserved in logs; these are required API semantics, not bypassed. Page-specific ICC/associatedalpha/compressed16/float and full TIFF variants remain unqualified.

2026-10-12 independent compressed TIFF precision: eight existing tifffile/imagecodecs-produced Deflate TIFF uint16/float32 files, both byte orders, original and last-page one-low-bit changed variants; all24 selected pages match authored RGB component bits and opaque alpha exactly, retain3-page metadata and ICC. Native output then ICC preparation,32 reverse2->0->1->2 swap restores,63x65 SDR/HDR transport exact, writes3/reads4/errors0 perfile and finalroot/queue0. Manifest source hashes verified. Explicit --metal-tiff-pages driver requires both tests, four classic/BigTIFF cases and all eight precision cases; actual gate131 ordinaryGPU+2manual passed/13 ignored, all executed suites verified. Evidence: research/tiff-compressed-precision-metal-2026-10-12.json and retained logs/report. Floatproducer0..1 does not qualify above-one HDR; distinct page ICC, associatedalpha and independent color correctness remain open.

2026-10-12 signed HDR TIFF and explicit linear policy: decode previously ignored assume_untagged_linear_srgb on unprofiled floatTIFF, retainingUnspecified and refusing display. Explicit opt-in now declares onlyfloatTIFF linear-sRGB; default unknown color still refuses preparation, integerpolicy unchanged, embeddedICC precedence tested with independent precisionfiles. Two authored DeflateRGBA32 TIFFs LE/BE, threepages each: RGBabove1/negative/signedzero, straight/associatedalpha, orientation6. Allnativebits match authored straight/oriented references; Metal63x65 HDR target containsfiniteabove1andnegativecomponents; SDR/HDR everycomponent equals independent authored raster. Pressure/cancel restores publishnothing then retryexact; reverse2->1->0->2 payloadbits/selection retained, writes3/reads4/errors0, finalCPU/queue0. Actual driver131ordinaryGPU+3manual passed/13ignored allverified; finaldecoder1012passed/72ignored. Evidence: research/tiff-signed-hdr-linear-policy-2026-10-12.json, authoredgenerator/manifest and retained failed/passinglogs. No independent HDRproducer/color, physicalHDR or full TIFFvariant qualification claimed.

2026-10-12 distinct page ICC qualification: authored3-page Deflate RGB8 TIFF contains identical codes with LittleCMS-generated sRGB, explicitly gamma1-edited/independentlyparsed RGB profile, sRGB. Native ICCbytes differ onlywhereexpected, first/thirdprofiles/preparedframes identical, middle differs. Both fallbackflags do notoverrideICC. Independent LittleCMS relative-colorimetric RGB8 oracle agrees after preparedfloat->sRGB encoding within1code; initial idealcurve0.00015 testfailed and preserved, replaced by actual-profile quantizedoracle ratherthan claiming exactICCfloatprecision. Four reverse1->2->0->1 swaprestores retainselection/floatbits and63x65 SDR/HDRexact; writes3/reads4/errors0; finalCPU/queue0. Fixture/oracle/manifest regenerate bitidentically with normalizedICCtimestamp/ID. Updated explicitTIFFgate requires4manualtests; actual131ordinaryGPU+4manualpassed/13ignored allverified. Evidence: research/tiff-distinct-page-icc-2026-10-12.json. Full floatingICCaccuracy, arbitraryprofiles and physicalHDR remain unqualified. Production unchanged from final1012decoderpass revision.

2026-10-12 viewer TIFF selected-page integration: actual production raster loader/cache/preload helpers retain3 page-specific keys for one TIFF with distinctICC.96byte/count3RAM, full-root-pressure RAMhits bypassdecoder,1secondTTL spillsall3 backgrounds; count0RAM then reverse1->2->0 swap preloads bypass heldforegrounddecoder ticket (watchdog). Payload/metadata/128x96MetalSDR/HDR exact, writes3/reads3/errors0, finalroot0. CPU-only marker now reports payloadonly; Metalmarker separately requiresactualadapter andSDR/HDR. All214ordinaryapp passed/60ignored; explicitTIFFgate131ordinaryGPU+4fixturemanual+1viewerhelper passed/13ignored allverified. Evidence: research/tiff-viewer-selected-cache-2026-10-12.json and preserved logs (initial misplacedtest compile failure retained). This is productionhelper integration, not liveUIeventloop/workercollection or largeTIFF qualification.

2026-10-12 rapid page navigation fix: App previously based every bracket-key step on lastdisplayedindex, so whilepage1loaded asecondforward againrequestedpage1 and reversefromvisible0 didnothing. Generation-bound pendingtarget now accumulates0->1->2 and reversesbeforecompletion without changingvisibleindex; currentterminalevent/newfile clearsit, cancellation/obsoletegeneration ignored. Scientificwindow adjustment retainspendingpage. HeadlessactualApp/queue regressions cover0->1->2->1->0, boundary, cancelledtokens, staleRasterReady/Failed, currentfailure reset. All216ordinaryapp passed/60ignored; separatelyactualMetal TIFFviewer cache/TTL/blockdecoder helper passed1 with128x96 SDR/HDRexact/root0. Evidence: research/pending-page-navigation-2026-10-12.json. Livephysicalkeyboard/window interaction remains unverified.

2026-10-12 selected ICO images: native bounded directory selector provides depth/area-ranked imageindices, preservingcodec defaultbest/tiebreak; validates4096-entry cap and payloadbounds, managedshared-budget rank scratch released beforepixels. Redirects selectedrecord/count in existing admitted source and restoresheader afterresult, no fullfilecopy; selectedindex/count retained. Three PillowPNG authored entries prove exactRGBA/alpha/dimensions and rankorder, malformed/memoryrefusal cleanup; existingBMP-backed baseline remainspassing. ActualMetal64byte/count1RAM full-pressurehit, visibleTTL thenreverse2->0->1->2 swap keepsmetadata and63x65 SDR/HDRexact, writes3/reads4/errors0/finalCPUqueue0. Decoder1014passed/72ignored; app216passed/60ignored; combinedICO/TIFFgate131ordinaryGPU+6manual passed/13ignored allverified. Evidence: research/ico-selected-images-2026-10-12.json and source/fixture/log hashes. Multi-DIB/masks, embeddedcolor and multiCUR remain unqualified; ICOrow not fully qualified.

2026-10-12 selected CUR and hotspot tie fix: boundedmanagedICOselector now supportsCUR, validateshotspots and ranksbyarea without treatinghotspotY asbitdepth. Selectedentry pixels/point/imageordinal/count staytogether; header restored afterdecode. Legacycursor normalizer usedlastmaxarea hotspot whileICOcodec couldchoosefirstmaxarea pixels whenlastdirectoryentrywassmaller; tie rule corrected toactualcodecdefault. Authored3PNG-backedentries (twolargesttieddifferentpoints+smalllast) reproduceandprotectthiscase; invalidhotspot refusal/root0/inputunchanged. Decoder1016passed/72ignored, app216passed/60ignored. Actual combinedgate131ordinaryGPU+7manual passed/13ignored allverified: CUR/ICO64byte/count1RAM fullpressurehits, visibleTTL, reverse2->0->1->2 restorekeepspoint/metadata and63x65SDR/HDRexact/finalrootqueue0. Evidence: research/cur-selected-hotspots-2026-10-12.json and hashes/logs. Multi-DIBmaskvariants andliveOS cursorpositioning remainunqualified; CURrow notfullyqualified.

2026-10-12 CUR DIB selected-image qualification: new authored24bitDIB fixture includes padded rows and AND-mask transparency, independently read by Pillow as ICO. Three selected CUR images retain native pixels, matched hotspots, selection metadata through RAM64bytes/count1, full-pressure hits, visibleTTL and reverse swap; exact63x65SDR/HDR on Metal, finalroot/queue0. All3ICO/CUR integration tests passed; driver now requires third DIB workload marker/adapter. Combined actualMetal gate passed. Evidence: research/cur-dib-mask-metal-2026-10-12.json and hashed logs/fixtures. Other DIB bitdepth/mask variants and liveOS cursor placement remain open.

2026-10-12 CUR DIB depth extension: authored1/4/8bit indexed,16bitRGB555 and32bitBGRA three-selected-entry fixtures independently decoded per-entry by Pillow. Exactnativepixels, matching hotspots, partial32bitalpha orANDmask transparency survive64byte/count1RAM/fullpressurehits/visibleTTL/reverse swap and exact63x65MetalSDR/HDR; finalroot/queue0. Combined gate135passed/13ignored, all executed suites verified; driver requires allfive depth markers and adapters. Evidence: research/cur-dib-depths-metal-2026-10-12.json. Uniform-color authored fixtures do not qualify arbitrary palette packing, compression/bitfields, mixed per-pixel masks or liveOS cursor placement.

2026-10-12 mixed CUR DIB coverage: six9x3 authoredfixtures (1/4/8/16/24/32bits) vary pixels across packedbyte/row boundaries with bottom-up rows, mixedANDmask and32bitpartialalpha; per-entryPillow decode independently validates expectedRGBA. Native pixels/hotspot and exactMetalSDR/HDR preserved through full-pressureRAMhit, TTLexpiry andswap; finalroot/queue0. Combinedgate136passed/13ignored, all executed suites verified. Twoinitialtest setupfailures retained: ZEROttl beforehit andfixedexistingTTLdeadlines; corrected sequencing, no productionchange. Evidence: research/cur-dib-patterns-metal-2026-10-12.json. UncompressedBITMAPINFOHEADER only; compression/bitfields/extendedheaders/liveOScursor placement remainopen.

2026-10-12 ICO/CUR viewer selected cache: generalized existingTIFF application qualifier to production load_cached_raster_mode/load_raster_for_display_typed/preload_raster for ICO,PNG-CUR,DIB-CUR three selected entries. Separate keys survive full-rootpressure RAMhits withoutdecode,1sTTL,3swapwrites and reverseprefetch fromswap whileforegroundDecodeGate held; exactserializedpayload/hotspot and128x96MetalSDR/HDR,3reads/errors0/finalCPU0. App217passed/61ignored; combinedgate142passed/13ignored allverified (131GPU+5ICO/CUR+1ICO/CURviewer+4TIFF+1TIFFviewer). Evidence: research/ico-cur-viewer-selected-cache-2026-10-12.json. Headlesshelper evidence does not prove livekeyboard navigation.

2026-10-12 liveCUR keyboard navigation: current normalviewer onAppleM4Max/Metal SDRsurface(Bgra8UnormSrgb), cachecount1/TTL1s, actual bracketkey events producedindices0->1->2->1->0 with matching dimensions/windowtitles; targetedwindow screenshots captured (image2 visuallyinspected green). Viewerclosednormally/exit0. UnprofiledPNG-in-CUR strictmode refused as designed; explicit--untagged-color srgb enabled display. Evidence: research/ico-cur-live-navigation-2026-10-12.json and screenshots/logs. LiveSDR only; no inferredspecificswap-hit orHDRsurface/calibratedcolor claim.

2026-10-12 actual mainviewerHDR: AppleM4Max/Metal surfaceRgba16Float/ExtendedSrgbLinear selected three authoredfloatTIFFpages0->1->2->1->0 using keyboard; explicitlinear-sRGBpolicy/cachecount1/TTL1s. ActualAXwindowsizes640x480then900x650 confirmed; targetedpage3screenshots at bothsizes visuallyinspected correctrotated2x3content. Normalclose/exit0. Evidence: research/metal-live-hdr-viewer-resize-2026-10-12.json and screenshots/logs. This closes selected mainviewerHDR presentation/resize path only, not injectedpressure recovery, physicalHDR calibration or inferredswap-hit.

### PTX primary-source boundary (2026-10-12)

Official Pentax Photo Browser/Laboratory v3.10 manual lists PEF/DNG and no PTX match; K10D recording documentation names PEF/DNG. These bounded sources do not establish a distinct PTX storage contract or prove historical absence. Current PTX-to-PEF routing is compatibility only. Keep row55Pending/countsunchanged; producer-original fixture and storage evidence remain required. Evidence: `research/ptx-primary-source-boundary-2026-10-12.json`.

### JPEG XL retained-output admission (2026-10-12)

JXL now reserves final RGBA capacity before pixel decode/conversion and adopts the reservation into retained pixels. 8/16-bit shortage/exact-budget/shared-owner cases pass alongside precision and ICC/alpha checks. Full decoder regression: 1018 passed, 0 failed, 72 ignored. Codec header/internal scratch is not covered by this retained-output reservation. Report: `research/jxl-output-admission-2026-10-12.json`; catalogue status unchanged.

### 2026-10-12 — JPEG XL codec working admission

Extended the earlier output-only fix: separate working credit now covers a tracker cap of min(65536 + 64 bytes/pixel, MAX_RASTER_BYTES), plus one output-image equivalent for conversion overlap. The cap is enforced through JXL decoder limits and the shared memory budget reserves working credit before pixel decoding. An output-only budget now correctly refuses; enough working/output credit succeeds and retains only output credit after decode. Three focused tests and 1018 ordinary decoder tests passed (72 ignored). Actual Metal collection passed 65 sources, 94 selections, 188 native/prepared storage cases, final root/queue zero. Report: `research/jxl-working-admission-2026-10-12.json`. Header parsing and untracked codec allocations are outside this reservation; it is not a strict RSS guarantee or universal JXL variant qualification. This supersedes the earlier output-only admission boundary.

### 2026-10-12 — grayscale JXL viewer recovery and decode-error cleanup

Expanded Metal collection with alpha-container-1.jxl and precision16.jxl plus independent libjxl native RGBA oracles and exact authored 16-bit samples. The first run failed at display preparation: Gray ICC was categorically rejected. Fixed color preparation to transform replicated grayscale samples through Gray ICC into linear sRGB, with Gray tone-curve normalization, unchanged alpha and refusal of nonreplicated inputs. Mathematical gamma-2 control passes. A truncation regression tests all 249 nonempty prefixes, requires at least one error after working admission and verifies final managed bytes zero after every attempt. Final decoder suite: 1020 passed/0 failed/72 ignored. Actual Metal collection now passes 67 sources, 96 selections, 192 native/prepared cases; final root/queue zero. Failed reproduction and successful logs hashed in `research/jxl-gray-icc-recovery-2026-10-12.json`. Broader gray ICC variants and calibrated physical display qualification remain open.

### 2026-10-12 — grayscale ICC through ordinary PNG/TIFF decode

Added an ordinary file-route regression for PNG La16 and TIFF Gray16 carrying a Gray ICC gamma-2 profile. Native samples 0/32768/32769/65535 remain exact; linear RGB matches the independent mathematical squared-normalized-sample control within 0.0001, adjacent values remain distinct and PNG 16-bit alpha is unchanged. All managed credit is released. Focused test and full decoder suite passed: 1021 passed, 0 failed, 72 ignored. Evidence: `research/gray-icc-png-tiff-2026-10-12.json`. This validates one authored profile curve, not all ICC variants or physical HDR display behavior.

### 2026-10-12 — authored grayscale PNG/TIFF across RAM/swap/Metal

Added deterministic independently byte-authored Gray ICC gamma-2 PNG La16 and TIFF Gray16 fixtures (generator `scripts/generate-gray-icc-fixtures.py`; regeneration hashes match). Metal collection verifies native samples, mathematical gamma-2 linear values, distinct adjacent 16-bit values, alpha, native/prepared RAM/swap and exact SDR/RGBA16Float transport. Actual Apple M4 Max passed 69 sources, 98 selections, 196 storage cases; final root/queue zero. Evidence and source hashes: `research/metal-gray-png-tiff-2026-10-12.json`. This covers one authored profile curve; broader producers/profiles and physical calibrated HDR output remain open.

### 2026-10-12 — independent LittleCMS grayscale color oracle

Built a separate LittleCMS C oracle using the embedded authored Gray gamma-2 ICC profile, relative-colorimetric intent and double-precision linear-sRGB output. Current normal decoder/display dumps for both PNG La16 and TIFF Gray16 agree at all four authored gray values; maximum absolute linear RGB error 1.1920928955078125e-7. Adjacent 32768/32769 outputs remain distinct and PNG alpha remains normalized source coverage. Evidence, oracle source and hashes: `research/gray-icc-lcms-2026-10-12/report.json`. This is independent CPU color evidence for one profile/four values, not all ICC variants or physical HDR calibration.

### 2026-10-12 — complete 16-bit grayscale range vs LittleCMS

Independent CPU oracle comparison now covers all 65536 source codes in a 65536x1 PNG alpha16 row, across every 4096-pixel transform chunk boundary. Authored gamma-2 ICC: maximum linear RGB error 1.7881393432617188e-7; every adjacent red pair strictly increasing. Authored 17-entry sampled Gray curve: maximum error 1.537799835205078e-5 (1.007797 linear 16-bit code units), no decreasing pairs; both native and LittleCMS produce 16383 equal adjacent red pairs. The initial strict-distinctness requirement for sampled curves was invalid and rejected; no native-only precision defect was established. Alpha error <=2.9801867640344426e-8. Evidence, generator and independent C oracle: `research/gray-icc-full-ramp-2026-10-12/report.json`. Two authored curves only; broader ICC variants and physical HDR remain unqualified.

### 2026-10-12 — DPX/Cineon explicit untagged viewer policy

Extending the shared Metal collection with all 24 DPX and 12 Cineon native-oracle fixtures exposed refusal despite explicit untagged sRGB selection. Fixed DPX and the shared Cineon envelope path to accept requested sRGB/linear-sRGB only if both transfer and colorimetry are undefined (255); declared incompatible color and strict default remain refused. Ordinary regression verifies both policies and declaration boundaries. Full decoder suite: 1022 passed, 0 failed, 72 ignored. Actual Metal collection: 105 source files (not formats), 134 selections, 268 native/prepared RAM/swap cases, final root/queue zero; DPX/Cineon native samples match FFmpeg/OpenImageIO references. Evidence includes failed reproduction: `research/dpx-cineon-untagged-metal-2026-10-12.json`. Explicit interpretation is not producer-color or film-log qualification; broader variants and calibrated HDR remain open.

### 2026-10-12 — DPX/Cineon buffer admission before allocation

Fixed DPX output allocation before managed admission: final RGBA credit reserves before pixel storage and is adopted into output; 8-bit conversion also reserves the overlapping u16 working buffer. Exact u8 capacity avoids retaining a wider reused allocation. Cineon now admits its exact envelope header/payload copy in the same root before allocation. New 8/16-bit tests verify one-byte-short refusal releases all credit, exact-budget peak and only retained output credit after decode. Full decoder: 1024 passed, 0 failed, 72 ignored. Actual Metal collection remains green at 105 sources/134 selections/268 cases, final root/queue zero. Report: `research/dpx-cineon-buffer-admission-2026-10-12.json`. Managed admission is not a strict OS RSS guarantee.

### 2026-10-12 — direct 8-bit DPX output

Removed the full-image u16 intermediate and subsequent conversion for 8-bit DPX (also used by Cineon). Pixel components now write directly into admitted final RGBA8; higher precision writes admitted RGBA16. Pixel-buffer peak falls from 12 to 4 bytes/pixel, excluding source and Cineon envelope. Exact-budget regressions prove only final pixel credit is needed; short budgets release cleanly. All 24 DPX/12 Cineon independent sample controls pass. Full decoder 1024 passed/0 failed/72 ignored; actual Metal collection 105 sources/134 selections/268 cases remains exact with final root/queue zero. Evidence: `research/dpx-direct-output-2026-10-12.json`. Throughput was not measured, so no speedup claim.

### 2026-10-12 — measured DPX 8-bit row fast path

Added direct 8-bit nonrotated row handling for gray/RGB/RGBA/ABGR, preserving shared header/output admission and checking cancellation in <=4096-pixel blocks. General orientation/packing path remains. All 1024 decoder tests pass (72 ignored). Added authored 4097x2 RGB8/padded-row fixture and exact native oracle; actual Metal collection passed 106 source files, 135 selections, 270 cases, final root/queue zero. On the same authored 12MP RGB8 source, saved-before/new binaries ran ABBA sequentially, each three warmups plus eleven samples: load p50 before 283.361/305.919ms, after 84.398/82.908ms (~3.52x ratio of mean run medians); CPU-ready p50 before 375.655/383.941ms, after 173.600/164.812ms. Prepared hashes match in all runs; managed peak remains 240000000 bytes. Dev workspace opt-level=1/dependencies=3, warm filesystem/shared host; not a universal/release/GPU speed claim. Evidence and binary/source hashes: `research/dpx-row-fast-path-2026-10-12/report.json`.

### 2026-10-12: full 12MP DPX Metal/swap output

Actual Apple M4 Max Metal passed native and prepared storage for an authored 4000x3000 RGB8 DPX: every native sample matched the authored oracle, and every SDR / RGBA16Float output pixel matched before and after swap. Lease protection, cancellation after restore allocation admission, budget refusal/retry, and final managed root/queue zero passed. Full-output test: 1 passed, 0 failed (13.81 s). Shared collection regression: 106 sources, 135 selections, 270 storage cases; 1 passed, 0 failed (23.17 s). Evidence: `research/metal-dpx-12mp-full-frame-2026-10-12/report.json`. RGB8 content qualifies HDR output transport only; physical HDR calibration, NVIDIA hardware, OS RSS and physical slow-disk pressure remain outside this check. Format qualification counts unchanged.

### 2026-10-12: native BAY image-only row streaming

Added `read_bay_image_sensor_with_budget` and explicit `image_dimensions`: QV5700 produces2576x1924 image-bearing sensor samples without decoding trailer positions or allocating a full-storage intermediate/crop copy; legacy2585-wide full-storage API is preserved. QV2000UX/QV3000EX retain1632x1211/2080x1547. Every new output sample matches pinned authored independent LibRaw test-only dumps; self-contained ordinary tests also verify all authored values. Exact output+one-row budget passes; one-byte-short refusal releasesall; QV5700 peak9,915,680bytes/finalzero. Focused9 passed; full decoder1024 passed/0 failed/73 ignored. Evidence: `research/bay-image-only-stream-2026-10-12.json`. Initial fixture path error corrected and failed log preserved. Original camera interoperability, CFA/WB/color and viewer routing remain Pending; catalogue counts unchanged. Layout source: https://www.inweb.ch/foto/rawformat.html.

### EXR physical pixel geometry — 2026-10-14

The bounded single-part header pass now retains finite positive `pixelAspectRatio` independently of color interpretation. Its definition is physical pixel width divided by height ([OpenEXR technical introduction](https://openexr.com/en/latest/TechnicalIntroduction.html#header)). Generic raster orientation and display preparation preserve the field; sample dimensions and every float bit remain unchanged for 2:1, 1:2 and 1:1 controls. Zero, negative, NaN, infinity, duplicate, wrong-type and wrong-length declarations reject explicitly. Native file errors and successful frames release all managed credits. Before-fix regression observed None instead of 2.0; eight focused tests and full ordinary decoder suite (1,035 tests) pass, with 73 manual/corpus cases ignored. New EXR live-window/GPU coupling is not yet exercised by this change; deep/multipart/full qualification remains open. Evidence: research/exr-physical-pixel-aspect-2026-10-14.

EXR geometry follow-up: the native decoder → RAM → swap → prepared Metal route now passes 2:1, 1:2 and 1:1 controls. Source deletion prevents rereading the original; restore pressure/cancellation retries preserve every float bit and complete SDR/HDR frames. Each case writes/reads once and releases managed root/queue credit. The full ordinary HDR/swap harness passes 23 tests, with 3 manual/external cases ignored. This is offscreen geometry/transport evidence, not new live-window or full EXR qualification. See research/exr-physical-pixel-aspect-2026-10-14/metal/result.json.

### HEIF decoded pixel spacing — 2026-10-14

HEVC decoding now retains the existing backend decoded-image pixel spacing in the common raster and display preparation, after libheif has applied crop/rotation/mirroring. Zero decoded spacing rejects explicitly. Three metadata-only authored HEIF fixtures preserve compressed media and all decoded samples/dimensions/color; no second image transform is applied. Native API2:1 previously became absent. Generator byte reproduction, eight focused tests, full ordinary decoder1,036passes (73manual/corpus ignored) and production check pass. This is decoded-backend metadata transport, not broadderived-item/old-backend or newGPU/live-window qualification. AVIF remains subject to its distinct square-pixel constraint ([AVIF specification](https://aomediacodec.github.io/av1-avif/)). See research/heif-physical-pixel-aspect-2026-10-14/result.json.
