# Expanded layered raster cache/swap/Metal qualification

The prior1940-case legacy raster GPU corpus did not include Aseprite or PSD/PSB. Expanded its independent sample inputs with9 PSD/PSB manifest compositions (raw/RLE/ZIP/prediction and merged alpha) and60 Aseprite selected-frame cases. Aseprite includes explicit sRGB, linear and assumed-sRGB policies; its3 ICC-profile frames retain dedicated coverage instead of deriving a circular profile oracle here. Pixel references come from existing external/authored manifest dumps, not a production decoder baseline. Catalogue scope/count is unchanged.

CPU pipeline and actual Metal M4 Max both pass2009 cases. Actual GPU run12054 SDR/RGBA16Float readbacks: oracle/newly loaded/restored outputs compare exactly.2009RAM hits pass while all available root credit is reserved;2009swap writes and2009restores pass with the decoder gate blocked, final root used0. Original caps remain: root131072bytes, RAM8192/count1, queue65536/count4, restore65536, disk1048576/count2048. Full viewer276 passed/74ignored; actual Metal test separately enabled and passed in23.08seconds. The initial expected count missed the merged-alpha PSD (2008 instead of2009); corrected assertion and failed log preserved.

This is authored-fixture native/prepared cache/swap/GPU transport qualification. It does not cover all PSD/PSB layers or producers, tag-directed playback, physical HDR, native window pacing or NVIDIA/CUDA.

## All Aseprite manifest frames including ICC

Independent direct chunk extraction confirms the ASE ICC equals separately pinned dicom-rgb-profile.icc byte-for-byte (588bytes,SHA256997e3ae554d84879d0e8588b4ecc73b4509270526d2543603887c08d62180f85). Expected color metadata now reads that separate fixture instead of the production Aseprite parser. Added the remaining3ICC selected frames: corpus2012 cases, including all63Aseprite manifest states and9PSD/PSB cases.

CPU and actual M4 Max Metal pass under original caps.12072SDR/float readbacks compare exact oracle/load/restore pixels;2012RAM pressure hits,2012swap writes and2012decoder-blocked restores;final0. Actual GPU run29.06seconds. This result includes current PSD bounded ZIP/borrowed raw plane/ICC reservation and Aseprite ICC ownership changes. Full parallel viewer276 passed/74ignored. No physical HDR scan-out, native animation timing or deferred NVIDIA qualification claim.
