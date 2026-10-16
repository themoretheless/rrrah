# Original repository RGB16 PSD/PSB fixtures

Pinned psd-tools repository commit9ea48dad04a32a6977b3109ebfae23b667590d42 supplies unmodified16bit5x5.psd and16bit5x5.psb (22592/23482bytes). SHA256 values and source URLs in provenance.json; repositoryMITlicense retained. These are original repository source files; the application that produced them was not independently verified.

Both are5x5RGB16 raw composites with3channels, no merged transparency. Each has75stored RGBu16samples,60of which are not exact8bit expansions; opaque alpha adds25derived samples. The standard-library Python oracle generator independently walks declared sections, extracts big-endian composite planes and ICC resource1039, writes200byte RGBA16LE and3144byte ICC references, and verifies pinned source hashes before generation. It does not call the Rust decoder or psd-tools runtime.

Native decode matches200RGBAu16samples across two files and exact ICC bytes. Exact3344byte borrowed decode admission passes;1byte-short refuses; clones retain credit until last owner then return0. Public decode_image file routing classifies both as raster, preserves all bytes, and peaks at source size+3344bytes (25936/26826), releasing source credit on return. Layered66passed/2ignored; decoder193passed/5ignored. No production changes in this turn; fixtures, generator and regressions added.

16bitmerged transparency remains unsupported and unqualified: these two files do not contain it. Display color preparation, cache/swap/Metal for these original files still need qualification. Catalogue remains94implemented subsets/6Pending/0fullyqualified.

Regenerate: `python3 scripts/generate-psd16-producer-oracles.py`.
Primary source: https://github.com/psd-tools/psd-tools/tree/9ea48dad04a32a6977b3109ebfae23b667590d42/tests/psd_files .

## Display cache / swap / actual Metal follow-up

Both original RGB16 sources now enter the shared corpus using independently extracted RGBA16 and ICC files as expected native input. Production display preparation succeeds, retains explicit profile interpretation (no assumed-sRGB flag), and cache/swap/GPU output matches the separately built expected raster path. The color transform is shared by expected and actual preparation; this is preservation/transport evidence, not independent CMS color accuracy qualification.

CPU and actual Metal M4 Max pass2014cases,12084SDR/RGBA16Float readbacks,2014RAM hits under full root pressure and2014decoder-blocked swap restores,final0. Root131072/RAM8192count1/queue65536count4/restore65536/disk1048576count2048 caps remain unchanged. Hardware run46.56seconds. No production changes; corpus tests extended and no extra default-suite run. Native u16 exactness remains established separately; GPU reference matching is not a claim that binary16upload preserves every u16code exactly. Physical HDR,NVIDIA,merged16alpha and allproducer variants remain open.


## ICC PCS D50 correction follow-up

Independent LittleCMS comparison found then fixed approximate-D50source/destination colorants. Both original RGB16profiles now meet unchanged2e-5RGB bound(max7.57e-6); current2014-case Metal and36MPICCasync chain pass after correction. Detailed evidence: [ICC PCS report](../icc-pcs-d50-correction-2026-10-16/REPORT.md). Earlier shared-transform transport checks remain distinct from this new independent accuracy evidence.
