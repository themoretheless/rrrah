# pi-dcp prompt defaults

This directory is REGENERATED on every pi-dcp init — do not edit files here.

To customize a prompt, copy it to `../overrides/` (same filename) and edit
that copy. Then enable `experimental.customPrompts: true` in your pi-dcp
config and restart pi.

## Files

- `compress-message.md`  — description text the LLM sees for the compress tool in message mode
- `compress-range.md`    — description text for compress tool in range mode
- `soft-nudge.md`        — default soft system-prompt addendum (used when compress.nudgeForce='soft')
- `strong-nudge.md`      — aggressive variant (used when compress.nudgeForce='strong')
- `hard-nudge.md`        — system-prompt addendum when context usage crosses the hard ceiling
- `iteration-nudge.md`   — system-prompt addendum after iterationNudgeThreshold messages since last user msg
