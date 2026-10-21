# Native EPS loop and exit

Frame continuations now explicitly distinguish ordinary procedure return, counted repeat, numeric for and indefinite loop. They live in the same pre-admitted execution-frame array. loop repeats its procedure until exit, cancellation or work exhaustion; even an empty procedure consumes frame steps. exit searches outward with charged/cancellable work, discards the nearest loop frame and nested procedure frames, and resumes its caller. Operand values are preserved. An exit with no active loop returns typed InvalidExit. No recursive calls or growable execution stack were introduced.

Tests verify conditional exit from loop, exit from for retaining counter values, inner repeat exit leaving the outer loop active, count-one repeat boundaries, and named/exec nested procedures. Additional checks prove empty-loop work exhaustion, invalid exit, cancellation after frame storage admission, same-root successful retry and finalzero ownership. All242vector tests pass/11ignored; public rrrah-decode library check passes. These are interpreter/recovery checks, not independent final pixels or GPU integration.

Independent test-only Ghostscript10.05.1 outputs match each selected stack trace: finite loop leaves3,9; nested nearest-loop exit yields22; for exit leaves1,2,3,9; count-one repeat exit through exec leaves9. ghostscript.log retains version and output. Existing numeric for traces and pixel regression remain in the suite.

This expands bounded native EPS execution. Full PostScript dictionary/array/string/font/image behavior, arbitrary EPS producers and full100format coverage remain incomplete. No physical HDR/NVIDIA claim follows.
