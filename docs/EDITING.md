# Native editing and recovery

The viewer has document/page-local exposure history. `+`/`-` changes exposure,
`Ctrl/Cmd+Z` undoes, `Ctrl/Cmd+Shift+Z` (or `Ctrl/Cmd+Y`) redoes. `R` resets the
view and records the exposure reset. Hold `B` for the initial exposure; `C`
toggles a shared-geometry 50/50 split. Neither mode decodes the sensor again or
allocates another image texture. Comparison currently changes exposure only;
it is not a comparison of different demosaicers or camera rendering profiles.

`Ctrl/Cmd+S` explicitly writes the source's `.xmp` sidecar. Unknown XML outside
the owned rrrah record and other page records are retained. An external change,
invalid/oversized document, busy writer or symlink is refused. Source image
bytes are never rewritten. Cooperative writers use a transaction lock; an
external editor that ignores it can race the final rename. A stale lock after
a killed writer is retained and reported, rather than automatically stolen.

Recent states are also saved asynchronously as checksummed, atomic snapshots in
`edit-journal` below the configured cache directory (default application cache).
Recovery verifies source identity, page, schema and checksum. A mismatched or
corrupt journal is retained and reported, not silently applied or deleted.
There is currently no dialog for choosing a conflicting recovery record; resolve
that record separately before retrying. Cache cleanup can remove these recovery
snapshots; explicitly save XMP to keep edits durably alongside the source.

History uses one shared 1 MiB allocation budget, at most 16 documents and 64
states per document. Busy/unsaved histories are not evicted. The native
`EditState` supports at most 32 monotone curve points; the existing CLI renderer's
256-point curve API is unchanged. A graphical curve editor is not included.

`H` reports a bounded source-linear RGB/luminance histogram for prepared float
rasters: at most 65,536 samples, 64 half-stop bins from -16 to +16 EV relative to
18% grey, with separate negative/zero/underflow/overflow counters. Nonfinite
pixels and invalid alpha are counted separately, transparent RGB is ignored.
Results from obsolete generations are discarded and source ownership is released
before notifying the UI. RAW GPU histograms and a graphical plot remain open.

The AccessKit bridge exposes the image, status announcements, selected folder and
eight keyboard commands. Tab/Shift+Tab selects a command; Enter/Space invokes it.
Native screen-reader and contrast qualification still requires a live GUI run.

For current qualification and open limits, see the first 30 entries in
[IMPROVEMENTS_1000.md](IMPROVEMENTS_1000.md). A build/test against a pinned coherent
core is separate evidence from a successful build of the live working tree.

Editing keys remain unavailable until source identity, XMP and recovery have finished opening. This prevents an early key press from replacing edits that are still being recovered. If both XMP and recovery exist with different states, the viewer retains both and refuses editing/saving until the conflict is resolved. A graphical resolution dialog is still pending; this condition is not silently resolved by choosing one file.
