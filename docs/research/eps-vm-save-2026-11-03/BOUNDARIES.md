# Protected graphics save boundaries

Follow-up to the checkpoint primitive. This still does **not** implement VM
`save`/`restore`, VM object rollback or original Illustrator Tiger support.

The saved stack now stores state plus a VM-boundary flag in the same admitted
buffer. Checkpoints retain both, so restoring a checkpoint from before an inner
boundary removes that boundary without losing an outer one. Boundary pushes
use the existing saved-state limit and allocate no additional buffer.

`grestore` restores but retains a boundary. The new native `grestoreall` operator
scans to the nearest boundary and restores it without popping it. With no
boundary it restores the bottom saved state and clears the stack; on an empty
stack it has no effect. It retains persistent geometry and paint output.

The scan checks cancellation/work before publication. The VM bridge preserves
`Limit(Work)` versus `Cancelled` rather than converting exhausted work into a
graphics cancellation error. Limits and cancellation leave graphics state and
stack contents unchanged. Future VM integration must capture a checkpoint
before pushing a boundary, validate VM references before restore, and use that
checkpoint only when committing its corresponding VM rollback.

Specification: Adobe [PLRM3](https://pdfa.org/norm-refs/PLRM.pdf), operator
descriptions `grestore`, `grestoreall`, `save` and `restore` (printed pages
603, 648-649 and 653). Tests cover nested boundaries, repeated grestore,
checkpoint boundary retention/removal, empty stacks, full-stack refusal, every
scan cancellation point and every insufficient VM work allowance.

No new format count, producer-original compatibility, speed, RSS or hardware
qualification is claimed. Earlier REPORT/source hashes describe the preceding
checkpoint commit; this follow-up has separate source hashes and test evidence.

Executed regression: 264 vector tests passed (11 ignored), 364 ordinary viewer
application tests passed (109 ignored). Three new focused controls passed.
Formatting and whitespace checks passed. Source hashes, commands and terminal
summaries are in [boundaries-result.json](boundaries-result.json). Full repeated
regression logs are not duplicated here. This is specification-based validation;
no new independent Ghostscript run or GPU/HDR qualification was performed.
