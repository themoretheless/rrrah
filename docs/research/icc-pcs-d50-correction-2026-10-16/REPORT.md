# ICC PCS D50 source/destination correction

Independent LittleCMS2190double RGB oracle exposed a real display-preparation error on pinned RGB16PSD/PSB: maximum absolute RGB error0.0003815293312072754 exceeded the predeclared2e-5bound. Source/native precision and cache/GPU transport had passed, so those checks did not establish CMS accuracy. Test-only C oracle uses independently extracted u16planes/profile, standard D65sRGBprimaries, linear destinationTRCs, relative-colorimetric intent and NOOPTIMIZE. Oracle f64LE files/hashes/sourceC and regeneration commands are retained alongside original-fixture research; no C dependency enters production.

Cause: approximate chromaticity/CCT D50 in generated colorants differed from conventional ICC PCSXYZ[0.9642,1,0.8249]. Added shared Rust rrrah-vector::icc_colorimetry helper with f64primary construction, Bradford adaptation to conventional PCS, consistent header/mediawhite/chad, CICPclearing and finite/singular validation before mutation. Generic raster destination uses it. An initial destination-only fix revealed that PNGgAMA/cHRMsource-profile synthesis also used approximateD50; migrated that source builder to the same helper. Existing explicit source ICC bytes remain untouched. Primary contract: https://www.color.org/security/malformed/bad-illuminant/ and https://www.color.org/chadtag/ .

After: both original files maximum RGB error0.000007569790, exact alpha;2e-5bound unchanged. Full PNGu16gamma/chromaticitypower controls pass (including wide gamut/custom white). Synthesized profiles encode exact conventional PCS header values; nonfinite and singular inputs leave previous profile unchanged. Initial invalid-input test incorrectly classified finite fictitious primaries as universally invalid; corrected the test rather than reject supported wide-gamut coordinates. All failing logs preserved.

Validation: default workspace libraries1630passed/100ignored across20binaries; viewer276passed/76ignored. Actual M4 Max2014-case cache/swap corpus12084SDR/float readbacks passes, including pressure RAMhits and decoder-blocked restores,final0. Separate actual36MPICC+9MPneighbour/asynchronousResidentSource/segmentedcrop+720pLODqualification42readbacks passes with unchanged720288588byte peak and quota/cancellation guarantees. This does not qualify allICCclasses,16bitmergedalpha, physical HDR, native window scheduling or deferred NVIDIA/CUDA;100-format goal remains incomplete.

Oracle regeneration (test-only, installed LittleCMS):
`clang -O2 -I/opt/homebrew/include docs/research/psd16-original-producer-2026-10-16/lcms-oracle.c -L/opt/homebrew/lib -llcms2 -o /tmp/rrrah-psd16-lcms-oracle`
Then invoke with `<source.icc> <source.rgba16le> <source.linear-rgba64le>` for each pinned source.


## Full 16-bit grayscale ICC follow-up

Pinned gamma-2 grayscale ICC was checked over all 65,536 native u16 codes against independently generated LittleCMS 2190 double linear-sRGB output (relative colorimetric, NOOPTIMIZE). Regenerating the reference with the retained C oracle produced byte-identical output. C/LittleCMS is test-only.

The native display path compared 196,608 RGB samples: maximum absolute error 1.78814e-7 against the unchanged 2e-5 bound. All adjacent codes were strictly increasing in every RGB channel. Varying alpha was preserved exactly. Native plus prepared retained credit was 1,573,056 bytes and returned to zero after dropping both owners. The full animation library suite passed 30 tests, with one ignored. This qualifies this grayscale profile and precision path, not arbitrary ICC profiles or physical HDR output.

Evidence: `gray-full-u16-result.json`, `gray-full-u16.log`, `gray-animation-suite.log`, `gray-ramp-oracle.c`.


Full-u16 grayscale admission follow-up: exact tracked pixel/profile/row peak 3,670,208 bytes. One byte of concurrent pressure causes a typed memory refusal; failed conversion releases all temporary credit and preserves every original u16 sample. Releasing pressure permits retry at the exact cap with unchanged independent color accuracy; final credit is zero. Full animation suite: 30 passed / 1 ignored (`gray-budget-final.log`). CMS internal allocator overhead is outside this tracked pixel/profile/row budget.


Full-u16 grayscale cancellation: all 68 observed cancellation checkpoints were interrupted independently, spanning setup, gray validation, input rows, transform chunks, output copying, and final publication. Every interruption returned typed cancellation, retained only native pixel/profile credit, preserved all 65,536 source codes, and allowed a bit-exact successful retry at the same 3,670,208-byte cap. Final credit zero. Animation library: 31 passed / 1 ignored. Evidence: `gray-cancellation.log`, `gray-full-u16-result.json`. Profile parsing and transform construction remain indivisible operations, and CMS internal allocations are outside pixel/profile/row accounting.
