# Native bounded EPS dictionaries and dictionary stack

The VM now implements name-keyed `dict`, `begin`, `end`, `currentdict`, `known`,
and dictionary forms of `get`, `put`, `length`, in addition to the existing
array forms. `def`, lookup and bind resolve through the active dictionary stack
before the initial user table and supported system operators. Copies of a
handle share the same dictionary, and `eq` compares identity. New dictionaries
have distinct identities even when empty. Initial size is a capacity hint;
record growth is allowed within explicit implementation limits.

## Ownership, limits and rollback

The initial hashed user table remains unchanged for scalar-only programs.
Dynamic dictionaries use one lazily admitted fixed shared record arena; each
identity/header and definition consumes one record. `max_dictionary_entries`
bounds this arena separately from the initial user table. This prevents one
allocation per dictionary or unbounded growth. Name searches charge work per
record and compared byte. Dynamic searches are currently linear in the shared
used prefix; this change does not claim a throughput improvement.

The stack is lazily admitted, has fixed capacity and is bounded by the caller's
`max_execution_frames`, including the initial permanent user dictionary. `end`
cannot remove that permanent entry. The caller's root budget charges backing
capacity, not merely the number of occupied records. Returned evaluation owners
retain dynamic dictionary backing; exposing `currentdict` also conservatively
retains the initial user table through the last shared evaluation owner. This
is managed payload accounting, not a physical RSS claim.

VM snapshots copy admitted dictionary records and their used prefix. Restore
rolls definitions back but preserves the actual dictionary stack. It validates
surviving operand and dictionary-stack handles first; a reference to a discarded
dictionary refuses with `InvalidRestore`, including an empty dictionary. Active
procedure references reachable through saved dictionary definitions are included
in the existing source-procedure lifetime checks. Graphics and VM publication
remain behind their cancellation/validation checkpoints.

Supported keys are literal names. Full `systemdict`, `where`, `store`, `undef`,
`forall`, arbitrary key types, access controls and stream-aware PostScript remain
unimplemented. Unknown behavior refuses. AI remains Pending and catalogue counts
do not change.

## Evidence

Twenty CC0 source cases were executed in separate top-level Ghostscript 10.05.1
processes. The fixture pins the executable, image, source and generator hashes;
no procedure/stopped wrapper changes allocation time. The native VM matches
numeric/boolean outputs and the specific `undefined`, `dictstackunderflow` and
`invalidrestore` errors. Cases include scope, aliases, growth, bind lookup,
rollback, retained pre-save dictionary stacks and discarded references.

Four new vector controls also cover every exposed cancellation point, every
insufficient work allowance, exact payload quota/one-byte-short refusal, record
and dictionary-stack limits, and final root-credit release. Full regression
results and source pins are in [result.json](result.json).

The authored 8x8 dictionary-prolog fixture retrieves an RGB array from a named
dictionary and mutates it across nested saves, with dictionary begin/end scope.
Ghostscript RGB matches all 64 analytic pixels (retained green/blue output,
restored red array). The native decode/RAM/disk-swap/pressure/cancellation pipeline
and exact SDR/HDR offscreen Metal checks use the same fixture. See the retained
hardware log and result for actual outcomes; physical HDR and NVIDIA remain
outside this check. Reproduce the reference with
`python3 scripts/generate-eps-dictionary-reference.py` and Docker available.

## Producer original

The same external, hash-pinned Illustrator 1.2d4 Tiger now passes `dict`, initial
`load begin` and early procset definitions. The rebuilt public decoder refuses
at instruction 50, `where`, instead of the previous instruction-six `dict`.
[Diagnostic](original-tiger.json). No artwork or substitute preview is vendored.
Full Illustrator/AI support and the original 100-format/cache/GPU objective
remain open.

Executed gates: 274 vector tests passed (11 ignored), 364 ordinary viewer tests
passed (109 ignored), and the new lifecycle test passed on Metal Apple M4 Max.
Production check, public decoder example build, formatting and whitespace checks
passed. Only the short [hardware log](metal.log) is retained; repeated full
regression logs are represented by their terminal summaries and digests.
Contract reference: Adobe [PLRM3](https://pdfa.org/norm-refs/PLRM.pdf), dictionary
and dictionary-stack operator descriptions. These bounded native gates do not
qualify the full PostScript interpreter or the complete requested format set.
