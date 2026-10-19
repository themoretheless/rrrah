# Rejected borrowed-edge interval candidate

Candidate replaced four per-leaf copied edge-parameter arrays with borrowed sorted-table slices. Geometry, emission order, tolerances and limits were unchanged. Interpreter134passes/1ignored and vector187passes/10ignored; all four fixture pixel hashes match the preserved baseline and all accounted peaks are unchanged.

The first six-pair run overlapped a project test build and is retained as exploratory-overlap-timing, excluded from performance conclusions. A second six-pair run had no project compilation overlapping the benchmark. Warm filesystem,3warmups/9timed load+prepare iterations per process, dev optimized/shared host; hashing, startup, destruction and GPU excluded. Source/binary hashes and raw rows are in timing.json.

Second-run median paired ratios: flat0.9933, Tensor0.9850, Coons0.9872, VectorApple0.9799. These medians conceal material regressions: VectorApple candidate/baseline ratios [0.9678,0.9660,0.9795,0.9802,2.1630,2.1027]. Last two candidate p50s921.381/957.190ms versus baseline425.982/455.225ms. The cause is unproven; host interference is possible but cannot be asserted as the explanation. No reliable end-to-end speedup is established.

Candidate rejected and adaptive.rs restored byte-for-byte to its saved pre-experiment source, identical to committed main for this file. Candidate patch and tests remain research artifacts, not shipped code. The preceding production Coons fix remains in the worktree, with its prior exact analytic, refusal/retry and actual Metal RAM/swap evidence. No fresh Metal run is necessary for a rejected implementation; the restored implementation is unchanged. Broader performance/format/cache/GPU qualification remains open.
