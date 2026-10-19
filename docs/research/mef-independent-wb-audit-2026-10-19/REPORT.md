# Independent MEF white-balance reference audit

The existing pinned MEF investigation reports camera multipliers [0,1,0,0], while the later metadata audit found no usable gains in its seven non-text MakerNote blocks. Those are bounded prior file observations; this turn did not rerun sensor decoding or assert every file region lacks WB.

Fresh source inspection of LegacyDenoiser revision1b346c05524e21c4c784406dd49991196824a151 confirms explicit Mamiya ZD/.mef support. Its DNG conversion reads camera WB and requires positive components; otherwise it derives a D65 neutral from its color matrix and later writes AsShotNeutral. That fallback is an assumed illuminant, not independent evidence of this source camera's chosen WB. It therefore cannot qualify the missing native as-shot color route. No converter was executed and no dependency was added.

The fpexif repository revisionc4b85b9bd100f1717a64b9f18df26c4453100418 advertised three Leaf/Mamiya samples, but its tracked download helper contains instructions/placeholders rather than a new MEF fixture URL. This audit did not inspect every external repository mentioned by that project or establish that no other originals exist.

The photographer's current Mamiya review links the original Windows PhotoStudio archive and LegacyDenoiser. Neither software was installed or run. A primary producer/reference result with documented source WB remains a possible next route; the archive link alone is not such a result.

The reproducible read-only script records exact URLs, source hashes and checked branches in result.json. Search snippets, supported-format lists and preview-derived WB are not pixel/color oracles. MEF remains pending; no format count increase is claimed.

Primary sources:
- https://github.com/kristoffer-trolle/LegacyDenoiser/blob/1b346c05524e21c4c784406dd49991196824a151/src/legacy_denoiser/dng.py
- https://github.com/pgray/fpexif/blob/c4b85b9bd100f1717a64b9f18df26c4453100418/test-data/download-samples.sh
- https://www.kristoffertrolle.com/2024/mamiya-zd-digital-back-review/
