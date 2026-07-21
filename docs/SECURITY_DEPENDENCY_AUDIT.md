# Security and dependency audit

Date: 2026-07-21  
Scope: the locked six-crate workspace, all target-specific dependencies, release and CI configuration.

This is a supply-chain review, not a claim that the current prototype is safe to
feed arbitrary files in-process. The decoder accepts attacker-controlled RAW
bytes and `rawler` can allocate proportional to dimensions and compressed
sections. A production viewer must keep the decoder behind a quota-enforced
worker boundary (preferably a short-lived subprocess) and treat every result as
untrusted until validated by the core invariants.

## Commands and reproducible result

The following commands were run with the checked-in `Cargo.lock` and current
tooling (`cargo-audit 0.22.2`, `cargo-deny 0.20.2`, `cargo-about 0.9.1`):

```text
cargo audit --json
  advisory database: 2026-07-17
  locked packages: 373
  vulnerabilities: 2
  unmaintained warnings: 1

cargo deny check advisories bans licenses sources
  advisories: FAILED (same 2 vulnerabilities + ttf-parser unmaintained)
  bans: OK (duplicate versions remain warnings; local path deps now carry an
    explicit `version = "0.1.0"` so wildcard policy stays blocking)
  licenses: OK after explicit MPL-2.0/LGPL-2.1 policy entries
  sources: OK
```

The advisory result is intentionally non-zero. No vulnerability is listed in
`deny.toml`'s `ignore` array and CI must continue to fail until an upstream fix
is available and verified.

## Findings

### RUSTSEC-2026-0194 / RUSTSEC-2026-0195 — quick-xml 0.39.4 (P0 supply-chain blocker)

The vulnerable package is not used by the RAW decoder. It is pulled into
`wayland-scanner 0.31.10`, which is a build-time dependency of the Wayland
stack used by `winit 0.30.13`:

```text
quick-xml 0.39.4
└── wayland-scanner 0.31.10 (proc-macro/build)
    └── smithay-client-toolkit / wayland-* / winit 0.30.13
        └── rrrah (app)
```

The two advisories are an algorithmic-CPU denial of service (quadratic duplicate
attribute checks) and unbounded namespace binding allocation in `NsReader`.
Both are fixed in `quick-xml >= 0.41.0`. The current Wayland scanner declares
`quick-xml = ^0.39`, and crates.io currently reports `wayland-scanner 0.31.10`
as the newest release, so `cargo update -p quick-xml` cannot legally select the
fixed major/minor version. A transitive lockfile edit would be invalid and is
not an acceptable mitigation.

The attempted update is reproducibly rejected by Cargo:

```text
cargo update -p quick-xml --precise 0.41.0
error: failed to select a version for the requirement `quick-xml = "^0.39"`
required by package `wayland-scanner v0.31.10`
```

Current exposure is reduced because the parser runs while generating Wayland
protocol bindings, not while opening a user's RAW file. It remains a release
blocker until one of the following is completed and tested:

1. upgrade `winit`/Wayland parents to a release whose scanner uses
   `quick-xml >= 0.41.0`;
2. consume an upstream scanner patch/release with a pinned commit and a review
   of generated bindings (do not silently replace the registry crate); or
3. for a platform-specific build that does not need Wayland, disable the
   Wayland feature and prove the resulting graph with `cargo tree --target all`.

Do **not** add either advisory to `ignore`, and do **not** pin an unreviewed git
fork merely to make the audit green. The CI job should keep the failing audit
visible and track the upstream issue in release notes.

### RUSTSEC-2026-0192 — ttf-parser 0.25.1 unmaintained (P1)

The dependency path is:

```text
ttf-parser 0.25.1
└── owned_ttf_parser 0.25.1
    └── ab_glyph 0.2.32
        └── sctk-adwaita 0.10.1
            └── winit 0.30.13
```

It is only used for optional Wayland font decoration, not RAW math. The advisory
has no patched `ttf-parser` release; `skrifa` is the suggested maintained
alternative. We cannot substitute it at the top level because `sctk-adwaita`
owns this dependency. Upgrade `winit`/`sctk-adwaita` when they migrate, or use a
platform profile without `wayland-csd-adwaita` after measuring the UI impact.
This warning remains visible in CI; it is not an allowlist candidate.

### rawler 0.7.2 — LGPL-2.1 and untrusted decoder boundary (P0/P1)

`rawler` is the core RAW implementation and is distributed under LGPL-2.1
(the crate metadata uses the deprecated SPDX spelling `LGPL-2.1`). The explicit
allow entry in `deny.toml` is a legal policy decision, not a security waiver.
Release artifacts must include the exact LGPL text, source/relinking offer and
the dependency notices. `NOTICE` names rawler and the upstream repository;
packaging still needs a generated third-party license report (for example with
`cargo-about`) and a check that the report is shipped next to binaries.

`RawlerDecoder` catches Rust panics, but panic catching cannot contain an OOM,
stack exhaustion, pathological decompressor or a process-wide allocator attack.
The production design therefore requires:

- a metadata and file-size/dimension preflight before calling `raw_image`;
- per-job input, output, CPU-time and wall-time budgets;
- a dedicated worker process (or equivalent OS sandbox) with RLIMIT/cgroup/job
  object limits and no network access;
- bounded concurrency (one or a small number of RAW workers, independent from
  UI/GPU threads) and cancellation by terminating the worker when cooperative
  cancellation cannot be observed;
- post-decode checks for dimensions, sample count, finite values, black/white
  level ranges and total byte accounting before publishing a tile.

### `option-ext` 0.2.0 — MPL-2.0 (P2 policy)

`option-ext` is a transitive dependency of `directories 6.0` on the application
path. MPL-2.0 is now explicitly allowed in `deny.toml`. Keep the license text in
the generated third-party report; do not copy this allowance to LGPL-dependent
code without a separate legal review.

### Duplicate versions (P2 maintenance/performance)

`cargo tree -d` reports duplicates including `bitflags` (1/2), `hashbrown`
(0.16/0.17), `syn` (2/3), `toml_edit` (0.22/0.25), `windows-sys` (0.52/0.59/0.61)
and `winnow` (0.7/1.0). Most come from platform backends or rawler's build
tooling and are not security findings. They increase compile time and binary
surface, so review them after parent upgrades; never force a semver-incompatible
unification merely to reduce the count.

## Critic review

The first-pass “make cargo-deny green” approach was rejected for four reasons:

1. ignoring the two quick-xml advisories would hide a real DoS, even though the
   vulnerable path is build-time only;
2. changing the lockfile to quick-xml 0.41 without upgrading
   `wayland-scanner` violates its declared semver requirement and is not a
   reproducible build;
3. allowing LGPL without shipping relinking/source notices creates a release
   compliance failure;
4. panic catching around `rawler` is not an OOM or decompressor sandbox.

The accepted policy is therefore “fail closed”: advisories and source checks are
blocking; unmaintained and duplicate warnings are tracked with owners; license
allowances are explicit and coupled to packaging tests.

## Required CI/release gates

Run these on every dependency update and before a release:

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-features --locked
cargo audit --json                         # fail on vulnerabilities
cargo deny check advisories bans licenses sources
cargo tree --target all -d                  # review new duplicates
cargo about generate about.hbs > THIRD_PARTY_LICENSES.html
```

The fixture/security suite should additionally fuzz TIFF/IFD bounds, CR2 JPEG
restart markers, DNG tile offsets/counts, integer overflow in dimensions and
malformed metadata. Fuzz workers must run with a bounded heap and wall clock;
an OOM or timeout is a failing test, not a retriable decode error.

## Upgrade watchlist

- `quick-xml`: >=0.41.0 through a Wayland parent release; verify generated
  protocol code and rerun the full matrix.
- `winit` / `smithay-client-toolkit`: migration away from unmaintained
  `ttf-parser`, and platform feature changes.
- `rawler`: newer release, license identifier correction, parser hardening and
  tile/metadata APIs needed for quota-enforced decode.
- `wgpu`: security fixes and device-loss behavior; keep exact versions locked
  and run GPU validation tests on each supported backend.

Advisory database freshness is part of the build provenance. Record the audit
database commit/date in CI artifacts so a green result is reproducible.
