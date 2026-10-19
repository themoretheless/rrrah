# Ordinary EXR paired CPU timing

Two 1024x1024 FLOAT RGBA OpenEXR files, explicit Rec709/D65, uncompressed, contain signed HDR values and alpha 1 or 0.5. The reference source generator is retained; source data hashes are recorded.

The before binary replaces only the EXR alpha block with the block from 8ebce8658; all other current source is identical. Current source was restored byte-exactly before building the after binary. Both use stable dev optimized profile and managed 512 MiB admission. Four alternating AB/BA pairs per fixture, three warmups plus seven measured iterations per invocation. Warm filesystem/process, no decoded cache, CPU only. Prepared pixel BLAKE3 is exact across every run. Concurrent unrelated machine activity is uncontrolled.

Median of per-process CPU-ready p50: opaque 24.9610 → 25.9175 ms (+3.83%); fractional alpha 23.7675 → 26.5028 ms (+11.51%). This is a regression signal on bounded synthetic files, not a universal throughput result. No improvement claim. The added scan/unassociation path needs optimization and renewed paired measurement while preserving associated emission and overflow behavior. Managed memory releases to zero in each invocation.

An initial run could not execute copied binaries because copyfile omitted executable permission; executable bits were restored before any measurements.
