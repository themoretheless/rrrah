# Dependency update, security and MSRV audit

**Audit date:** 2026-07-21  
**Workspace:** `rrrah`  
**Rust toolchain observed on the audit host:** `cargo 1.96.0`, `rustc 1.96.0`  
**Workspace MSRV:** 1.89 (`Cargo.toml`)

This is a resolution audit, not a blind `cargo update`. The lockfile is committed and
must remain the source of truth for release builds. Every candidate below was checked
against the crates.io index and then classified by API risk, MSRV and runtime impact.

## Baseline

The workspace manifest deliberately pins the high-risk parts of the image path:

```toml
rawler = "=0.7.2"
wgpu = "=30.0.0"
winit = "0.30.8"
```

The lockfile currently resolves the following versions:

| Package | Manifest requirement | Locked version | Current crates.io candidate | MSRV | Result |
|---|---:|---:|---:|---:|---|
| `wgpu` | `=30.0.0` | 30.0.0 | 30.0.0 (stable) | 1.87 | No update |
| `winit` | `^0.30.8` | 0.30.13 | 0.30.13 (stable), 0.31.0-beta.2 | 1.70 / 1.85 | Keep 0.30 |
| `rawler` | `=0.7.2` | 0.7.2 | 0.7.2 (stable) | 1.88 | No update |
| `wayland-scanner` | transitive | 0.31.10 | 0.31.10 (stable) | 1.71 | No update |
| `quick-xml` | transitive | 0.39.4 | 0.41.0 | 1.56 / 1.79 | Blocked by `^0.39` |
| `pollster` | `^0.4` | 0.4.0 | 1.0.1 | 1.69 | Major update; defer |
| `image` | transitive (`rawler`) | 0.25.10 | 0.25.10 | — | No update |
| `tempfile` | `^3.23` | 3.27.0 | 3.27.0 | — | No update |
| `thiserror` | `^2.0` | 2.0.19 | 2.0.19 | — | No update |

The lockfile is already at the latest stable version for `wgpu`, `winit`, `rawler`,
`wayland-scanner`, `image` and `tempfile` available on the audit date. The `winit`
manifest lower bound is older than the locked version because Cargo's caret range
allows all compatible `0.30.x` releases; the actual tested API is 0.30.13.

## Commands and observed resolver behaviour

The following commands are the reproducible audit record:

```text
cargo update --workspace --dry-run --verbose
  Locking 0 packages to latest Rust 1.89 compatible versions
  Unchanged pollster v0.4.0 (available: v1.0.1)

cargo update -p quick-xml --precise 0.41.0 --dry-run
  fails: wayland-scanner v0.31.10 requires quick-xml = "^0.39"

cargo update -p wayland-scanner --precise 0.31.10 --dry-run
  no-op: 0.31.10 is already selected

cargo tree -i quick-xml --workspace --target all
  quick-xml 0.39.4
  └── wayland-scanner 0.31.10
      └── wayland-client / wayland-protocols / smithay-client-toolkit / winit
```

Do not add `quick-xml = "0.41"` to the workspace in an attempt to force this
transitive dependency. Cargo correctly rejects it because 0.41 does not satisfy the
`^0.39` requirement, and a direct dependency would not replace the proc-macro path.

## Candidate decisions

### wgpu

`wgpu 30.0.0` is the current stable release selected by the manifest. The upstream
30 release introduced substantial API changes (surface acquisition result enum,
explicit display handles, optional bind-group layouts, `WriteOnly` mapped buffers,
new depth/stencil option types, asynchronous adapter enumeration and MSRV 1.87).
Those changes are already reflected in this tree. There is no `wgpu 31` release on
crates.io as of this audit; a future major must be treated as a migration, not as an
automatic update.

Upstream release notes: [wgpu releases](https://github.com/gfx-rs/wgpu/releases),
[wgpu changelog](https://github.com/gfx-rs/wgpu/blob/trunk/CHANGELOG.md).

**Decision:** keep `=30.0.0`; upgrade only in a dedicated API/benchmark change. The
exact pin is intentional for GPU shader/layout stability, but it must be revisited if
a security patch for the 30 line appears.

### winit

`0.30.13` is the latest stable 0.30 release and is what the lockfile already uses.
`0.31.0-beta.2` is a pre-release with a split `winit-core`/backend architecture and
the event-loop redesign. It is not a semver-compatible update and should not enter a
release build. It also keeps the Wayland dependency chain, so it does not solve the
`quick-xml` advisory.

Upstream: [winit releases](https://github.com/rust-windowing/winit/releases),
[event-loop redesign](https://github.com/rust-windowing/winit/issues/2900).

**Decision:** keep 0.30.13. Consider raising the manifest lower bound to
`winit = "0.30.13"` after the next lockfile refresh; this is a reproducibility
improvement, not a functional requirement.

### rawler

`rawler 0.7.2` is the latest stable release and has an MSRV of 1.88. It is already
exact-pinned because decoder output and metadata semantics are part of the golden
corpus contract. The crate is LGPL-2.1; the repository's explicit `NOTICE` and
`PATENTS.md` handling must remain in release artifacts.

Upstream: [rawler source and release metadata](https://github.com/dnglab/dnglab),
[rawler 0.7.2 manifest](https://docs.rs/crate/rawler/0.7.2/source/Cargo.toml.orig).

**Decision:** keep 0.7.2. A future update requires CR2/DNG corpus comparison, not just
`cargo test`.

### pollster

`pollster 1.0.1` is available, while the workspace intentionally remains on 0.4.0.
The project only uses `pollster::block_on`; the common API is unchanged, but the
0.x-to-1.x transition is a semver-major update and must be tested explicitly. It has
no measurable effect on the RAW hot path because it only waits for GPU setup futures.

**Decision:** defer. If adopted, change the manifest to `pollster = "1.0.1"`, run the
full test/clippy matrix and compare startup benchmarks. Do not update it merely to
remove the dry-run notice.

## Security finding: quick-xml 0.39.4

`cargo deny check advisories` and `cargo audit` report:

* [RUSTSEC-2026-0194](https://rustsec.org/advisories/RUSTSEC-2026-0194): quadratic
  duplicate-attribute checking in `BytesStart::attributes()`;
* [RUSTSEC-2026-0195](https://rustsec.org/advisories/RUSTSEC-2026-0195): unbounded
  namespace-declaration allocation in `NsReader`;
* [RUSTSEC-2026-0192](https://rustsec.org/advisories/RUSTSEC-2026-0192):
  `ttf-parser 0.25.1` is unmaintained and has no safe version upgrade.

Both advisories are fixed in `quick-xml >= 0.41.0`; the upstream 0.41 changelog
documents the linear duplicate check and namespace declaration cap:
[quick-xml changelog](https://github.com/tafia/quick-xml/blob/master/Changelog.md).

The dependency is reached through the build-time Wayland code generator:

```text
quick-xml 0.39.4
└── wayland-scanner 0.31.10 (proc-macro)
    └── wayland-client / wayland-protocols / winit
```

The separate unmaintained-crate warning is reached through the optional Wayland
Adwaita CSD font path:

```text
ttf-parser 0.25.1
└── owned_ttf_parser 0.25.1
    └── ab_glyph 0.2.32
        └── sctk-adwaita 0.10.1
            └── winit 0.30.13
```

RustSec lists [`skrifa`](https://crates.io/crates/skrifa) as the maintained
alternative, but `sctk-adwaita` does not currently offer a skrifa-based feature or a
compatible release. Replacing this transitive font stack locally would be a
platform/backend fork, not a dependency update. The same build-time/runtime
reachability analysis and a time-bounded exception policy apply until upstream
migrates.

`wayland-scanner` parses bundled protocol XML while compiling; the application does
not feed user RAW, XMP or sidecar XML into this parser. This makes the reported remote
attack path non-reachable in the current app, but it does not make the vulnerable
crate acceptable as a general-purpose runtime dependency. The dependency remains a
release hygiene issue and CI should keep reporting it until upstream raises the
requirement.

### Remediation options (in order)

1. **Preferred:** wait for `wayland-scanner`/`wayland-client` to require
   `quick-xml >=0.41`, then update the lockfile and run the complete quality and
   cross-platform matrix.
2. **Platform build without Wayland:** for a product build that intentionally supports
   X11 only, use `winit` with `default-features = false, features = ["x11"]`. A
   temporary copy of this workspace confirmed that `cargo tree -i quick-xml` then
   has no result. This is not the default product configuration because it removes
   Wayland support.
3. **Local vendoring:** only if a release is blocked by the advisory, carry a small
   reviewed patch to the Wayland scanner's dependency and record the exact upstream
   commit. Do not use an unreviewed `[patch.crates-io]` or a manually edited lockfile.
4. **Scoped exception:** if policy requires a green `cargo deny` before option 1 is
   possible, an explicit, time-bounded ignore may be used only with an issue link,
   owner and removal date. It must state that the crate is build-time-only in this
   graph. A permanent ignore is rejected by this audit.

## Critic review

* **A direct quick-xml dependency is not a fix.** Cargo's resolver enforces the
  transitive `^0.39` range; forcing a different version would require a reviewed
  upstream-compatible patch.
* **A beta is not an update.** winit 0.31 changes backend crates and event-loop APIs;
  wgpu major releases change shader and surface contracts. Both need migration plans.
* **MSRV is part of the release ABI.** `rawler 0.7.2` requires Rust 1.88 while the
  workspace advertises 1.89. Any dependency update must be tested with the declared
  MSRV, not only the developer's Rust 1.96.
* **Lockfile freshness is not security proof.** `cargo update --dry-run` only tests
  semver-compatible candidates. It will not cross 0.x/1.x or major boundaries and it
  cannot infer whether a transitive parser is reachable from untrusted input.
* **Feature pruning trades support for attack surface.** Removing Wayland eliminates
  quick-xml from this graph, but it is a product/platform decision, not a generic
  security patch.
* **Exact pins need a review clock.** They protect shader/decoder reproducibility but
  can suppress future patch releases. Each pinned package needs a scheduled upstream
  review and a corpus/benchmark gate.

## CI gate for dependency changes

Run the following in the dependency-update job and attach the outputs to the change:

```bash
cargo metadata --locked --format-version 1 >/dev/null
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo deny check advisories
cargo deny check licenses bans sources
cargo audit --deny warnings
cargo tree -d --workspace --target all
```

For a decoder or GPU dependency change additionally run the golden RAW corpus,
shader validation and cold/warm benchmark jobs described in
[`QUALITY_BENCHMARKS.md`](QUALITY_BENCHMARKS.md) and
[`BENCHMARK_MATRIX.md`](BENCHMARK_MATRIX.md). A dependency update is not complete
until first-visible latency, steady frame time, RSS/VRAM and pixel-delta gates are
recorded on the same hardware labels.

## References

* [Cargo dependency specification](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html)
* [Cargo update command](https://doc.rust-lang.org/cargo/commands/cargo-update.html)
* [RustSec advisory database](https://rustsec.org/)
* [winit 0.30 documentation](https://docs.rs/winit/0.30.13/winit/)
* [wgpu 30 documentation](https://docs.rs/wgpu/30.0.0/wgpu/)
* [quick-xml 0.41 documentation](https://docs.rs/quick-xml/0.41.0/quick_xml/)
