# Native where and implemented system namespace

`where` searches name-keyed dictionaries from the current stack top, then the
initial user dictionary and the implemented system namespace. A hit returns the
actual dictionary handle and true; a miss returns only false. Ordinary name
lookup uses the same search and returned value, without probing the hit a second
time. This is a structural change, not a throughput benchmark claim.

The implemented system namespace has a distinct read-only handle and exposes
supported native operators and constants plus `systemdict`/`userdict` handles.
`get` and `known` work on that namespace. Pushing it with `begin` gives it normal
stack priority over lower user overrides. `def` and `put` targeting it refuse
with `InvalidAccess`. User overrides of operator names continue to work in their
own dictionaries; explicitly retrieving an operator from the system namespace
still yields that operator. Import-mode showpage retains its existing no-output
policy. This is not a complete PostScript system dictionary.

The system handle survives VM restore as static evaluator state; dynamic handles
still require pre-save lifetime validation. Exposing a user dictionary through
lookup or where now retains its backing via the ordinary push path. No new
unbounded stack or dictionary allocation is introduced. Searches charge work and
cancellation through the existing bounded VM, including key-byte inspection.
Memory claims refer to managed backing, not physical RSS.

## Qualification

Twenty CC0 top-level source cases were independently run in separate Ghostscript
10.05.1 processes. They cover missing names, nested scopes, aliases, user/system
overrides, operator retrieval, system-stack priority, read-only errors and
restore lifetime. The native outputs and specific error categories match.
Source, generator, image and executable hashes are pinned in the reference
fixture. The generator avoids a procedure/stopped wrapper that would change
object allocation time.

A second native control sweeps every execution cancellation point and every
insufficient work allowance, checking final credit release. System dictionary
size/enumeration is not fabricated: `systemdict length` explicitly refuses while
that complete contract remains unavailable. Unsupported operators, arbitrary
key types, access-control mutation and full stream-aware PostScript remain open.

The authored 8x8 EPSF AI fixture finds its RGB array through `where` in a nested
dictionary and retrieves its color operator from the system namespace. Nested
save/restore retains green/blue paints and restores the red array. Ghostscript
RGB agrees with an analytic map for all 64 pixels. Native decode, RAM cache,
disk restore under pressure/cancellation and exact SDR/HDR offscreen rendering
pass on Metal Apple M4 Max. These pixels do not qualify a physical HDR display,
NVIDIA hardware, all HDR content or general AI support.

Executed regression: 276 vector tests pass (11 ignored), 364 ordinary viewer
tests pass (109 ignored), and the new actual-Metal lifecycle passes. Production
check, public decoder example build, formatting and whitespace checks pass.
Only the short [hardware log](metal.log) is retained; [result](result.json)
records source pins and terminal summaries/digests without repeating full logs.
Reproduce the oracle with `python3 scripts/generate-eps-where-reference.py` with
Docker available. Contract reference: Adobe [PLRM3](https://pdfa.org/norm-refs/PLRM.pdf),
`where`, `known`, `load` and dictionary-stack operator descriptions.

## Producer original and next barrier

The same external hash-pinned Illustrator Tiger passes the former `where`
barrier. It next refuses while `/d /setdash ldef` loads the unsupported `setdash`
operator. The public error is `UndefinedName(19)`: that instruction is `load`
inside the reused `ldef` procedure, so its index is not the source call site.
[Diagnostic](original-tiger.json) includes the source/binary hashes and a minimal
public-decoder differential: loading setdash through ldef refuses, while loading
setlinewidth through the same procedure succeeds. The original artwork remains
outside the checkout. No dashed strokes or substitute preview are silently
accepted; native dash semantics and subsequent Illustrator operators remain
required. AI stays Pending, catalogue counts are unchanged, and the complete
100-format/cache/GPU objective remains active.
