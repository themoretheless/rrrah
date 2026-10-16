# Cancellation-aware partial RAW source reads

Shared managed whole-file RAW reads and both staged MEF passes now use a checked exact-read helper. It observes generation cancellation before and after every Read call, retries Interrupted with cancellation checks, rejects premature EOF, and checks cancellation after the final EOF probe before returning the managed source. The previous read_exact calls could repeatedly handle interruptions or short reads without checking the token; the final whole-file EOF probe also lacked a post-read cancellation check.

New deterministic helper tests cover cancellation on the third interrupted read, cancellation on the third one-byte partial read, short-input refusal, successful bytes, and empty EOF. Existing managed-source and MEF integrity/cancellation/release tests pass. Full RAW library: 436 passed, 59 ignored. Log: suite.log.

This cannot interrupt an operating-system Read while that individual call is blocked. Legacy unbudgeted reads, sniffing and the ZIP-owned EIP reader remain separate paths. It does not establish MEF WB/color qualification or physical slow-disk soak.


Unbudgeted RAW follow-up: the former read_to_end path now uses checked reads with a fixed 64 KiB scratch buffer and fallible Vec growth. It checks cancellation before opening and between partial/interrupted reads, while preserving the 2 GiB actual-source cap and acceptance of source growth within that cap. A one-byte over-limit probe is rejected before extending the retained source buffer. New tests exercise cancellation, declared-versus-actual growth, exact cap, empty input and zero cap. Full RAW suite: 437 passed / 59 ignored (`legacy-suite.log`). Individual blocking reads remain indivisible.


EOF publication regression: controlled reader changes generation during an EOF-returning Read; both shared checked read and unbudgeted whole-source import return Cancelled rather than success. Public read_bounded with an already cancelled missing path returns Cancelled in managed/unbudgeted modes, proving file-open is skipped; managed peak/final zero. Five focused I/O tests pass. An initial test compilation used unavailable tempfile; corrected using a unique nonexistent path without adding dependencies. Logs retained.
