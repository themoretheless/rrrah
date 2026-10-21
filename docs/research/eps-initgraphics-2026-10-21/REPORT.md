# Native EPS initgraphics

The new VM regression first failed UndefinedName(17); red.log preserves it. Native operator resolution/dispatch now resets only the active supported graphics State to its defaults. It clears CTM/path/clip and restores black, width1, cap0, join0, miter10 without erasing saved states, immutable nodes or earlier paints. Default abstract EPS coordinates still map through the document bounds/density transform.

Three regressions cover VM defaults and grestore recovery, direct path/currentpoint/clip clearing with unchanged arena credits and saved-node recovery, and exact full-document black pixels after clipping/color/singular-matrix reset at a negative origin with densities1and2. Managed credit returns tozero. Full vector suite235passed/0failed/11ignored. Existing independent gray and diagonal pixel compatibility gates remain unresolved.

Independent test-only Ghostscript10.05.1 output confirms width1, cap0, join0, miter10, gray0, empty currentpath, followed by restored currentpoint6,7, width4, gray0.75. The command configured those altered values, gsave, initgraphics, queried values/currentpoint via stopped, and grestore. Raw version/output is in ghostscript.log. This numeric/state oracle does not qualify arbitrary producer documents or independent edge coverage. No Ghostscript runtime dependency was introduced.

This expands the supported EPS subset only. Font/image/array/dictionary and other graphics-state semantics outside that subset remain unsupported; it is not full PostScript or100format completion.
