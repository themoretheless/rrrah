# Prepared CR3 paired CPU timing

The cr3_prepared_timing example compares NativeCr3Decoder.decode with prepare→decode on one existing local EOS R8 CR3, one process and one512MiB managed root. Sixteen alternating AB/BA rounds run; the first four pairs are warmup and excluded, leaving12 measured pairs. Outer wall time covers source reading and complete decode, not checksum validation or output destruction. All32 outputs have matching complete native-u16+metadata hashes, and managed used returns to zero after each output is released.

Stable optimized dev profile, warm local filesystem, CPU only, no decoded cache, GUI, GPU or LibRaw. This task ran no concurrent build/benchmark during measured execution; unrelated host activity is uncontrolled. Full-sample hashing after timing affects the inter-run workload, but applies to both variants in alternating order.

Direct median44.4169ms; prepared median44.0223ms. Median within-pair relative difference-0.609%, with individual differences ranging-10.07% to+5.20%. The small central difference is within observed variability: no speedup or statistical equivalence is claimed. This run found no substantial fitting-path slowdown on this source; it does not bound overhead for all metadata sizes, filesystems or CR3 variants.

Common managed peak73,371,346 bytes; final0. This is reservation accounting, not process RSS or independent pixel/color qualification. The previously proven partially available-cache refusal and snapshot ownership tests remain separate evidence.

Reproduce: CARGO_TARGET_DIR=/tmp/rrrah-alpha-isolated-target-20261016 cargo +stable run -p rrrah-decode --example cr3_prepared_timing --locked --offline -- tests/IMG_9043.CR3

Raw timings, source/benchmark hashes, hardware and worker environment are retained. No source media is distributed.
