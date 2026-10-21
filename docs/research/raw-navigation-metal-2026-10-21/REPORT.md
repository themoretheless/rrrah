# Current RAW worker and Metal navigation qualification

Both ignored hardware/fixture tests were explicitly executed with required Metal on AppleM4Max. Reversal passed in25.09s; multi-neighbor passed in9.11s. Exact logs, source/binary hashes and fixture hashes are retained. The four externally pinned decoded sources match their earlier recorded content hashes; the selected ARW hash is recorded but its pixels are intentionally not decoded.

Reversal uses two real EOSR8 CR3 files. A blocked forward generation is invalidated before publication; only the backward neighbor occupies the sole disk slot. Subsequent forward/backward changes replace that slot. Sensor pixels and metadata match fresh native decode; every1920x1080 and6188x4120 SDR pixel and scene-linear binary16 HDR component matches and remains finite. Managed roots return to0.

Multi-neighbor uses behind1/ahead2 around selected index2. Forward indices3,4,1 and backward1,0,3 each produce exactly three disk entries with zero failures. Across six checks, CR2/NEF/ORF/PEF sensor samples and metadata match fresh native decode,1920x1080 and fullsensor SDR pixels match, and fullsensor HDR components match and remain finite. Each managed root returns to0 after consumers release. The current test source confirms selected pixels are absent from speculative cache work.

This is transport equivalence to the same native decoder, not independent camera color or sensor-code qualification. This path uses DiskMosaicCache, not the separately qualified ImageSwapCache. Two complete neighbor plans do not establish mid-decode reversal of mixed cameras; CR3 reversal independently invalidates blocked pending work. No liveGUI, physical HDR, slow physical drive, all100formats, or NVIDIA claim follows.
