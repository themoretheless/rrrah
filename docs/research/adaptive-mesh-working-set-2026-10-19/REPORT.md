# Adaptive mesh working-set reduction without changing bounds

An ordinary regression reproduces the previous failure: the compact 32x64 parabolic Tensor mesh cannot be built under an8MiB admitted budget. The source uses the same32u / 2+8v+128u(1-u) construction as the analytical coverage control, with explicit f64 control points and solid RGB components. No PDF parsing/output buffers or reference-test allocations are attributed to the mesh budget.

Two allocation changes in shading/adaptive.rs:
1. Sort/deduplicate local edge coordinates, then compact their retained capacity before output allocation. Both old and replacement arrays remain admitted during compaction.
2. Count exact per-patch triangle fans from the finalized edge plan, without allocating per-leaf count arrays. Admit/allocate output capacity once per patch, including overlap with any existing output allocation. All triangle limits and cancellation checks remain enforced. Unmanaged Vec output preserves its old growth behavior; managed geometry/components are unchanged.

The same35584 triangles now build with4975184-byte peak accounted credit and3700736-byte retained output. Every f64 coordinate and active f32 source-component bit matches the generic adaptive representation. Limits remain geometry1/64 pixel, source components1/1024, depth12,1048576 maximum triangles. No accuracy or memory limit was relaxed. The test passes with a root limited to the measured peak, refuses one byte below it, and returns accounted memory to zero after both success and failure.

Validation: vector lib183passed/11ignored; internal Hayro interpreter133passed/1ignored, including cancellation and allocation-refusal retry tests. The workspace dependency cannot be tested directly via cargo -p because its dev-dependencies are outside workspace membership; the existing isolated manifest in target/ai-native-harness-20261013/hayro-interpret points at the actual vendored source. Manifest/lock/source hashes and logs are retained. Workspace format and diff whitespace checks pass.

Commands:
```
CARGO_TARGET_DIR=/tmp/rrrah-alpha-isolated-target-20261016 cargo +stable test -p rrrah-vector --lib --locked --offline
cargo +stable test --manifest-path target/ai-native-harness-20261013/hayro-interpret/Cargo.toml --lib --locked --offline
```

The after-focused log predates the additional exact-peak success assertion; the final vector suite includes that assertion. This measures accounted mesh allocation credit, not whole-process RSS or viewer latency. No speedup is claimed. The independent known fixed-grid coverage regression remains explicitly ignored because the optimized adaptive path is not yet connected to ordinary viewing. Next step is to reconnect production sampling and recheck full pipeline8MiB admission, pixel oracle, failure propagation and Metal/cache preservation. This result alone does not qualify AI, GPU/HDR or the100-format objective.
