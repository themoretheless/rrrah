# 100-format qualification target

The user requested correct operation with 100 image formats. This is an implementation target, not a measured global popularity ranking. Extension aliases (JPG/JPEG, TIF/TIFF) do not add entries. Container families with distinct animation, precision or storage contracts have separate qualification rows.

Completion requires actual library decoding and viewer opening, folder discovery, color management, orientation, precision/alpha preservation, bounded malformed-input handling, and licensed reproducible fixtures with pixel/metadata oracles per row. Animations need disposal, frame timing and loop semantics; layered/page formats need explicit selection/compositing; scientific volumes need explicit slice/window semantics. Camera RAW must decode the sensor, never substitute an embedded preview. Generic TIFF must remain separate from CFA DNG routing.

Current raster API decodes the first image by default. DCX supports explicit page indices, WAD3 selects texture entries, PVR/DDS/KTX1/KTX2 select mips, APNG/GIF/WebP/Aseprite select composited animation frames, and NRRD/MRC/FITS select scalar sections or planes and native DICOM selects scalar frames; other animation/page/layer behavior is pending. It preserves ICC bytes; `prepare_raster_for_display` applies RGB ICC profiles to linear sRGB, and marks unqualified PNG/TIFF/EXR color interpretation unspecified pending complete metadata handling. The common `decode_image` API distinguishes sensor and raster results, inspects TIFF sensor tags including SubIFDs, and never falls back from failed sensor decoding to a preview. CLI `--inspect` supports raster inputs; The window now routes prepared still rasters to a dedicated GPU renderer; mixed-folder navigation and color-managed raster thumbnails are connected. 90 of 100 rows have an implemented subset; 10 remain Pending (no qualified viewer implementation). None of these rows is fully qualified yet.

Reference format inventory: https://imagemagick.org/formats/ (used only as a format reference, not as a runtime dependency).

| # | Format | Current implementation | Completion evidence |
|---|---|---|---|
| 1 | JPEG | Raster first-image decoder; qualification pending | Pillow CC0 baseline pixel/alpha oracle passes; full variants/color/viewer pending |
| 2 | PNG | Raster decode, ICC bytes, sRGB/cICP precedence, straight alpha and 16-bit samples | Pillow CC0 baseline pixel/alpha oracle passes; full variants/color/viewer pending |
| 3 | GIF | Selected composited RGBA8 frames, delay and repeat metadata; decode stops at selection | Twelve independent Pillow presentations for KEEP/BACKGROUND/PREVIOUS; malformed bounds checked; live playback/full variants pending |
| 4 | WebP | Still decoder plus selected linear-light animation composition, delays/plays, ICC source color | Sixteen libwebp raw-frame/linear presentation contracts, lossy ALPH and lossless; full variants/live playback pending |
| 5 | AVIF | dav1d/image first image, ICC retained | RGB/alpha libavif oracles; ICC and NCLX sRGB display checked; HDR/crop/sequence pending |
| 6 | HEIF / HEIC | libheif HEVC primary image; ICC, NCLX sRGB/linear, straight alpha, item transforms and native precision | Four authored lossless pixel oracles and adjacent 12-bit values; other codecs/HDR/sequence/viewer/full qualification pending |
| 7 | JPEG-XL | jxl-oxide first frame, ICC and native sample precision | libjxl container/codestream alpha oracles; animation/HDR/CMYK/full qualification pending |
| 8 | TIFF | Raster first-image decoder; qualification pending | Pillow CC0 baseline pixel/alpha oracle passes; full variants/color/viewer pending |
| 9 | BMP | Raster first-image decoder; qualification pending | Pillow CC0 baseline pixel/alpha oracle passes; full variants/color/viewer pending |
| 10 | ICO | Raster first-image decoder; qualification pending | Pillow CC0 baseline pixel/alpha oracle passes; full variants/color/viewer pending |
| 11 | CUR | PNG/DIB image through ICO decoder; hotspot bounds checked | Two independent ICO pixel oracles; hotspot preserved through display; full cursor qualification pending |
| 12 | SVG | resvg shapes/text/embedded sRGB PNG, intrinsic viewport, straight RGBA | CairoSVG shapes and embedded-PNG alpha fixtures; other resources/animation/full qualification pending |
| 13 | EMF | Bounded 88/100/108-byte fixed header with optional UTF16 description rectangle/ellipse and 16/32-bit polygon/polyline/Bezier and continuation subset through SVG; fixed/anisotropic mapping with saved origins/extents, fill modes, MOVETOEX/LINETO current point, saved contexts, stock/custom solid/null RGB brushes/pens and bounded object table with deletion/index reuse | Eight authored opaque fills agree exactly with separate Java2D renderer and pass native/display, RAW-suffix routing, RAM leases, pressure-preserving streamed swap and Metal. Thirty mathematical fixtures pass RAM/swap/Metal; independent EMF-to-SVG/Cairo comparison is 25/30 exact with metric/reflected and continuation mismatches recorded; Bezier cases are collinear; the separate Java2D winding mismatch remains recorded. Independent producers, non-null stroke rasterization, transforms, clipping, fonts/bitmaps, extended headers/color and full qualification pending |
| 14 | APNG | 8-bit source frames, selected linear-light float32 composition, timing/repeat metadata | Authored linear-light oracles plus FFmpeg structural/compatibility readback; live playback/16-bit/full color and variant qualification pending |
| 15 | MNG | Native MNG-VLC plus admitted LC global palette/transparency and intrapixel subset; opaque 8/16-bit and linear-light alpha composition, explicit selection/ticks | Authored opaque and 28 alpha presentations preserve native/display samples through RAM/swap and actual Metal; embedded PNG samples independently agree with FFmpeg. Independent MNG compositor, wider control/loops/object placement and physical viewer qualification pending |
| 16 | JNG | Standalone 8-bit JPEG RGB/gray with PNG 1/2/4/8/16-bit or JPEG 8-bit straight alpha, outer color declarations | Eight Pillow JPEG/alpha sample contracts; native adjacent alpha16 preserved; wider JPEG/MNG/live viewer/full qualification pending |
| 17 | JPEG-2000 | OpenJPEG JP2/J2K, unsigned full-resolution RGB/gray/straight alpha, 8/16-bit, RGB ICC | OpenJPEG/FFmpeg pixel fixtures; palette/subsampling/signed/CMYK/full qualification pending |
| 18 | JPEG-LS | Static CharLS gray/RGB, 2–16 bits, planar/line/sample layouts, near-lossless, RGB ICC | 29 native-sample contracts: 13 FFmpeg readbacks, 16 authored lossless expectations; wider color/preset/SPIFF/interop/live viewer qualification pending |
| 19 | JPEG-XR | Static JXRLib gray/RGB 8/16-bit, HALF/FLOAT, separate alpha, ICC, all eight container orientations | 21 typed contracts; independent Rust non-alpha samples plus authored orientation; wider pixel layouts/interleaved alpha/color/interop/live viewer qualification pending |
| 20 | QOI | Raster decoder preserves sRGB/linear transfer flag and alpha | Both transfer flags tested through linear conversion; viewer/corpus pending |
| 21 | TGA | Raster first-image decoder; qualification pending | Pillow CC0 baseline pixel/alpha oracle passes; full variants/color/viewer pending |
| 22 | PNM | Raster first-image decoder; standard P1–P6 BT.709 preparation | ASCII/binary PBM/PGM/PPM and full-range 8/16-bit color checks pass; 65,536-value nominal reference and independently modeled Netpbm toe difference recorded; Metal frame and payload color-tag roundtrip pass. PAM, nonstandard linear/sRGB variations and broader qualification remain separate. |
| 23 | PAM | First image; RGB/gray/black-white with straight alpha and native maxval scaling | Five authored tuple/alpha oracles, including adjacent 16-bit values; transfer/page/viewer/full qualification pending |
| 24 | PFM | Native float RGB/grayscale, bottom-up, both endians; scale retained | Synthetic precision/row/endian/size tests pass; viewer and corpus pending |
| 25 | Farbfeld | Native RGBA16 retained; explicit assumed-sRGB import, straight alpha | Adjacent 16-bit sample oracle and linear-display alpha checks; compressed wrappers/viewer/full qualification pending |
| 26 | OpenEXR | First selected RGB(A) surface; HALF/FLOAT retained as f32 and associated alpha converted to straight | OpenEXR NONE/RLE/ZIP/PIZ pixel oracles; zero-alpha emission, explicit linear Rec.709 declarations supported; other primaries, layers/deep/tiled/viewer/full qualification pending |
| 27 | Radiance-HDR | RGBE float samples, unspecified linear RGB | Authored old-style and modern scanline RLE HDR oracles; primaries/exposure/orientation/viewer/full qualification pending |
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
| 39 | ASTC | Arm reference decoder, all 14 2D footprints; RGBA8 LDR / RGBA32F HDR, validated block classification | 14 reference pixel oracles; compressed and constant HDR precision checked; 3D/color semantics/viewer/full qualification pending |
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
| 62 | MEF | Pending: explicit native Mamiya ZD sensor import API | Pinned source public route now traverses linked SubIFDs but refuses unsupported TIFF field type 4084 before color; no preview fallback or retained managed credit. Public `decode_mef_zd_sensor` returns managed sensor codes only; full sensor comparison remains independent of viewer qualification. Native WB remains unresolved. Public TIFF structure and color support pending; see tests/fixtures/mef-investigation.json |
| 63 | ERF | Native Epson R-D1 subset; other profiles pending | CC0 object 2680: all 6,152,960 sensor samples and strict native color metadata qualified against LibRaw; persistent cache/swap exact roundtrip, pressure retry and whole-frame Metal readback verified. Five malformed variants rejected without preview fallback or retained managed memory. Physical display and universal ERF coverage pending; see `tests/fixtures/erf-qualification.json`. |
| 64 | IIQ | Native Phase One P20+-H format-3 subset; other cameras/storage pending | CC0 object 4366: all 17,065,152 samples after native black, pixel, gain-grid and isolated-column correction match independent LibRaw stages. Native WB/profile/crop, RAM leases, persistent cache, bounded swap pressure retry and whole-frame Metal readback verified. Seven malformed cases rejected with budget zero. Other variants, physical display and full photographic color pending; see `tests/fixtures/iiq-investigation.json`. |
| 65 | 3FR | Native uncompressed Hasselblad X1D full sensor with file-origin WB/matrix/levels/crop | Object 2058: all 52,852,736 samples match LibRaw. Embedded matrix precision and stored default crop retained; persistent cache, swap pressure retry and consistent 128×96 Metal frames verified. Stored matrix/WB/levels/crop independently match tifffile 2026.9.20. Photographic color accuracy and broader layouts pending. |
| 66 | FFF | Native CFV-50 predictor-8 sensor, native WB and exact FFF profile | Object 1637: all 51,679,680 samples and color match LibRaw; persistent cache, swap RAM-pressure retry and whole Metal frame verified. Broader camera/codec variants and full qualification pending. |
| 67 | X3F | Native Sigma SD10/SD14 legacy Huffman/CAMF subset, strict selected WB_DESC, linear RGBA16 and raster viewer routing | Pinned SD10 Auto 2267×1513 and SD14 Sunlight 2639×1757 public output match independent standalone converter channel-for-channel. SD14 explicit Auto is also exact. RAM/swap exact restoration, preload windows, generation cancellation and completed Metal rendering verified. Linear fixture full-frame Metal readback deviation <=1 code value. Other cameras/encodings and full-image qualification of additional WB modes remain pending, as do physical display/HDR; see `research/x3f-sd14-public-sunlight-2026-10-06.json` and `research/x3f-reproducible-full-output-2026-10-06.json` |
| 68 | RWZ | Pending | Pending |
| 69 | BAY | Pending: native explicit-layout sensor unpack for Casio QV-2000UX, QV-3000EX and QV-5700 | Camera-file qualification, CFA/color metadata and viewer routing pending |
| 70 | CAP | Pending | Pending |
| 71 | EIP | Pending: bounded inventory, managed RAW extraction and explicit sensor import APIs | Classic ZIP directory/entry admission and unique RAW candidate inspection; managed source inflation and IIQ sensor oracle; Capture One settings/ICC/LCC interpretation and real EIP oracle remain pending. |
| 72 | DCS | Native Kodak DCS520C lossless-JPEG TIFF subset, strict WB/profile/response curve | CC0 object 2573: original TIFF routes to backend 22 revision 1; all 2,013,760 sensor samples match LibRaw; managed ownership, RAM lease, disk/swap restore and Metal frame readback pass. Exact observed extra JPEG tail accepted only in this camera path; other cameras/tails, physical navigation/color/display and full qualification remain. |
| 73 | EPS | Pending | Pending |
| 74 | AI | Pending: `.ai` gallery candidate; PDF-compatible content uses the PDF renderer, including selected pages. PostScript AI remains unsupported. | Pinned Adobe-produced PDF-compatible AI: RAM/page keys and swap/Metal preservation pass, but strict Poppler comparison fails on two of three pages (33,054 and 2,790 differing pixels); one page exact. Rendering fidelity and PostScript AI remain pending. See research/current-ai-poppler-2026-10-06.json. |
| 75 | STI | Native indexed8/ETRLE, RGB16/24/32 masks and bounded zlib; explicit subimage selection, managed output | Ten authored images and twenty external JA2 images agree with sample references; external parser plus original blitter resolves literal-zero coverage; native/prepared RAM/swap/Metal checks pass. Independent RGB/zlib producers, placement/application semantics, color and full qualification pending |
| 76 | PDF | Hayro 0.7.1 selected page raster at one pixel per point, bounded geometry/page count, source/pixmap/final-buffer reservations and straight RGBA8 conversion | Two authored opaque 10x10 pages match Poppler and ordinary native routing exactly; managed ownership and insufficient budget/index refusals pass. Both selected opaque pages and a constant half-alpha page pass RAM leases, six native/prepared pressure-preserving streamed swap restores and actual Metal frame readback against mathematical colors. Parser/render scratch, interruptible rendering, transparency/fonts/color, independent producers and physical presentation qualification pending |
| 77 | GPR | Pending | Pending |
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
| 96 | RPF | Pending | Pending |
| 97 | PIC-Softimage | Native 8/16-bit RGB/alpha packets, raw/pure/mixed RLE; assumed sRGB | 12 OpenImageIO exact sample fixtures, malformed packets/runs and RAW-suffix routing; full variants/viewer pending |
| 98 | PIX-Alias | Native row RLE, 8-bit gray/24-bit BGR, assumed sRGB | FFmpeg pixel oracles, row bounds and maximum run checked; full qualification pending |
| 99 | WAL | Quake II base mip; explicit RGBA palette API or game-tree PCX palette | Two Pillow palette-override pixel oracles; game-tree routing and malformed mip/palette checks; full variants/viewer pending |
| 100 | WAD | WAD3 indexed mip textures with embedded palette and selected texture index | Two archives/four image oracles via vgio mip parser + authored Pillow palette; bounds/index/routing checks; full WAD variants/viewer pending |

Next work: qualify the connected common decoder/viewer/gallery against live opening, color and metadata contracts; implement the remaining codecs and per-format fixture gates; add animation, page/layer and scientific slice semantics. Update this matrix from executed evidence rather than compiled feature flags.

PFM contract: Debevec/Netpbm bottom-up raster; finite nonzero scale preserved separately as `DecodedRaster::sample_scale`, without altering samples. The indistinguishable Adobe top-down dialect requires an explicit import option (pending). Primary references: https://pauldebevec.com/Research/HDR/PFM/ and https://netpbm.sourceforge.net/doc/pfm.html .

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
continues refusing float records rather than guessing producer byte order.
Raw float channel lengths must equal active width times four; finite RGB values
including negative and HDR values survive unchanged. Alpha must be finite in
[0,1]; premultiplied interpretation refuses zero-alpha emission and numeric
overflow. Working/output pixels share one managed float allocation. A two-pixel
OpenImageIO-authored little-endian fixture and its independent readback agree
exactly with native HDR samples; an authored byte-swapped variant verifies Big.
Common request/CLI/cache-key routing for float byte order and GPU/swap testing
of this float-RLA path remain pending.
