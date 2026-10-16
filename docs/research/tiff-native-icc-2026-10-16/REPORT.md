# Native TIFF ICC slice extraction with early owned-copy admission

The generic TIFF path formerly requested ICC through tiff's get_tag_u8_vec, constructing a per-byte Value list and then a byte vector; getter failures were swallowed as no profile. New native extraction borrows ICC from the header-selected IFD before constructing the pixel decoder. It handles classic TIFF/BigTIFF, II/MM byte order, BYTE/UNDEFINED, inline and external storage, bounded directory counts, checked offsets/extents, cancellation and duplicate/empty/malformed profile rejection. Only one final Vec copy is fallibly allocated after managed admission, and its reservation is attached to shared color metadata. The public outer wrapper does not charge twice. Other TIFF pixels, alpha, orientation and page selection retain their prior decoder paths.

Parser tests cover8classic/BigTIFF/endian/inline-external combinations and truncated tables/data or wrong types. The pinned oriented RGBA TIFF control verifies exact588ICC+8inputpixels+8rotatedpixels peak (604bytes) both with borrowed source and public managed file input. One-byte-short refuses; metadata clones and separately retained pixels release the correct credits; exact alpha/pixel bytes remain[255,0,0,255,0,255,0,128]. Public decoder library: **205 passed /5 ignored /zero failures**. Viewer: **285 passed /80 ignored /zero failures**, including selected TIFF page cache/swap checks.

A standalone allocator probe compares the unchanged legacy image TIFF getter with the production borrowed parser and its single owned copy. Constructor/source storage is outside this metadata-only measurement; all results are byte-identical and measured private allocations release to0:

|ICC bytes|Legacy getter peak|Borrowed peak|Owned copy peak|
|---:|---:|---:|---:|
|65536|2162688|0|65536|
|4194304|138412032|0|4194304|

Getter uses3allocations; native borrowed parser0; owned copy1. Synthetic padded profiles preserve existing tags and update declared length. Public file decoding also succeeds at exact source+profile+16pixel-byte budgets (131862 and8389398bytes), retains profile capacity and1x2geometry, and releases root credit to0. This is requested Rust heap storage, not RSS or a latency comparison. Reproduction scripts, probe, logs and library/source hashes are retained; generated large inputs stay under/tmp.

Native invalid/duplicate/empty ICC declarations are now typed errors rather than silently discarded through the old getter. Supported profile field types are BYTE and UNDEFINED; no arbitrary malformed-field interoperability is claimed. Pixel codec-private state and other TIFF metadata remain outside complete shared-root accounting. This is not full TIFF/ICC color interoperability, physical HDR/NVIDIA or100-format qualification.
