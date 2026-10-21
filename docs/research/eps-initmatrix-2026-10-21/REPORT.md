# Native EPS initmatrix

Before implementation, the new vector regression failed with UndefinedName(14). The compiler/operator resolver now recognizes initmatrix and dispatches it through the ordinary budgeted/cancellable VM. The graphics state resets only the current abstract EPS user matrix to identity. Device mapping (EPS bounds, density and row direction) remains owned by the document rasterizer; paths already stored in device coordinates, clips, stroke/color style and saved graphics states survive. No extra allocations or external production decoder were introduced.

The vector regression checks an existing path through translation/scaling, currentpoint, linewidth/color, actual stroke path, and restoration of the prior transform through grestore. A document-level pixel regression checks negative EPS origin, densities1and2, persistent half-page clipping and recovery from a singular CTM. Exact authored output pixels match; managed memory returns tozero. Full vector suite:232passed/11ignored/0failed. Existing ignored compatibility gates remain ignored.

Independent Ghostscript10.05.1 numeric control uses: `7 8 moveto 3 setlinewidth 0.25 setgray 10 20 translate 2 3 scale gsave initmatrix currentpoint currentlinewidth currentgray count {==} repeat grestore quit`. Stack output is0.25,3,8,7, exactly matching native values in reverse order. It ran test-only in Alpine3.22.1 image sha256:4bcff63911fcb4448bd4fdacec207030997caf25e9bea4045fa6c8c44de311d1. This is a numeric operator oracle; it does not qualify independent stroke-edge pixels or broader EPS producers.

Known grayscale quantization and diagonal scan-conversion compatibility disagreements remain unchanged. Full EPS arrays/dictionaries/fonts/images and physical display/100-format qualification remain incomplete.
