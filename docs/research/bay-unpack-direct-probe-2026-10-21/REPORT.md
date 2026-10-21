# BAY direct-shift unpack pilot — not accepted

Compared current five-byte/u64 unpack against four explicit u16 shift/mask formulas for the QV5700 full-row storage layout (2585x1924). Both paths match all4973540deterministic samples including the final10-bit sample. The optimized standalone probe uses compile-time variants, preallocated output,12full-image decodes per observation, warmup,6AA controls and16alternating AB/BA pairs. Raw initial unwarmed/runtime-control observations remain separate and are not the accepted analysis.

Mean candidate time reduction -0.681%, bootstrap95% [-1.811406249929626, 0.22534618545363752]; AA control -0.115%, interval [-1.7441429709910912, 1.174946642425634]. No positive performance conclusion is supported and production source remains unchanged. The probe is standalone CPU evidence, not integrated decoder/disk/viewer speed or original camera support.

Source qualification remains separate: current BAY APIs explicitly require layout and do not infer CFA/WB/color. The checked author format-description page documents headerless dumps and model-specific geometry but provides no original BAY download link inspected in this pass: https://www.inweb.ch/foto/rawformat.html . This is not proof that original files are unavailable everywhere. Search queries included Casio QV5700 BAY sample/download, NOCOMP.BAY download/sample and Casio BAY on rawsamples.ch. No camera-produced sample was recovered in this pass.

The pinned MEF investigation separately reports unavailable WB [0,1,0,0] and zero examined maker fields; no neutral substitute or viewer route was added. Next useful format action needs a distinct camera sample with usable color metadata or progress on another incomplete decoder. The full100format objective remains open.
