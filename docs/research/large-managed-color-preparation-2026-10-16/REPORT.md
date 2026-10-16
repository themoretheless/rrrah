# Large managed display preparation

Reproduced a real boundary:36MP PSD native decode succeeded, but display color preparation refused OutputTooLarge because its unconditional512MiB float-output limit was smaller than576000000bytes. Managed requests now rely on their configured memory budget for output admission; unbudgeted callers retain the existing512MiB hard cap. Checked dimensions and per-allocation reservations remain unchanged.

Extended the public6000x6000PSD qualification into display preparation. Before log records the failure. After: every36million linear RGB/alpha pixels match an independent f64 sRGB formula within2e-7; exact720000000byte tracked peak consists of144000000native RGBA+576000000linear float. Dropping native retains only576000000bytes; dropping prepared returns0. A one-byte-short budget refuses conversion, retains only native ownership then returns0 after dropping it. The unbudgeted call still refuses the large output. Prior252000040byte native peak and native-short refusal remain checked.

Workspace default-feature library regression: 1623 passed/99 ignored across20 binaries. Full viewer276 passed/74ignored. Large test explicitly enabled separately. Initial failing synthetic file was verified by signature/dimensions/size/row pattern and removed; successful test removes its own file. No current36MP GPU/swap/physical HDR proof is claimed here. Parser/zlib/allocator overhead is outside tracked byte counters; this is not a measured RSS or end-to-end viewing latency result.

## Actual public PSD / display RAM / segmented swap / Metal crops

A new explicitly enabled viewer integration test creates36MP PSD, calls production load_cached_raster_for_display and verifies720000000byte native/color peak and576000000byte retained prepared frame. A RAM hit shares pixels with decode fallback disabled while all remaining root credit is occupied. Full payload512MiB refusal remains explicit; production SwappedRaster segmented storage writes576000064bytes instead.

Dropping the visible/prepared RAM owners releases the576MB frame, retaining only the small budgeted segment index. The synthetic PSD source is deleted before restoration. Three17x11regions at origin/center/far edge restore exact float bits using2992byte output and65536byte scratch peak. On actual Metal Apple M4 Max,12SDR/RGBA16Float readbacks compare restored crops to their pre-swap references. All tracked credit returns0 after store/index drop. Test passes in3.02seconds; full viewer276 passed/75ignored, with this manual test separately enabled.

Initial test authoring errors used the wrong SwapStore namespace/path argument and a nonexistent parent directory; corrected, logs retained. No production change was necessary for segmented storage. This is real public file/display cache plus production segmented codec/GPU crop integration, not native App window/viewport-controller automation, full576MB GPU texture upload, physical HDR scan-out or NVIDIA qualification. Oracle crop copies/GPU allocations are outside CPU tracked-root accounting.

## Crop exact quota, cancellation and retry follow-up

Extended the same actual PSD→display RAM→segmented store→Metal integration. Each of three17x11rectangles requires tracked peak68592bytes (including2992retained output and temporary decode/read scratch);68591bytes refuses with all temporary credit released. At exact peak a cloned crop retains2992bytes and prevents overlapping restoration from exceeding that child quota. Dropping final owner returns0.

Controlled cancellation is triggered only after observing full crop admission at68592bytes. It returns SwapError::Cancelled and releases both output/scratch reservations. Reusing the same child budgets restores successfully; SDR and RGBA16Float output compare to pre-swap references. Actual M4 Max test passes with24readbacks, three short-quota refusals, three after-admission cancellations and three GPU-verified retries, final root0. No production edits in this follow-up and no new default-suite run; native window/physical HDR/NVIDIA remain unqualified.

## Streamed and cached720p LOD with full-target Metal readback

First verified17x11streamed/cached levels (165072byte stream peak,6048byte cached peak), then expanded the same integration to1280x720. Pre-swap direct-source downsample is the exact reference. After deleting the source PSD and releasing576MBprepared RAM, streamed LOD output14745600bytes matches every reference float bit at exact14948096byte peak; one-byte-short refuses and releases all credit.

Store quota permits source+one exact LOD (600MB/count2, index300000bytes). A second request succeeds under14811200byte child quota, below the14948096byte streaming requirement, proving cached-level recovery rather than a full source-row reconstruction. Its observed peak14811136bytes includes retained output plus checksum scratch. Both streamed and cached results render through actual M4 Max at full1280x720 SDR/RGBA16Float targets:8LODreadbacks exactly match reference outputs. Including crop cancellation/recovery,32readbacks pass; final root0. This is a test-only extension; default suite not repeated. Native window/controller/physical HDR/NVIDIA remain unqualified.

## Large-source disk/count/index refusal matrix

Before successful segmented storage, five fresh stores reject this actual576000064byte source record under disk0, disk576000063(one byte short), count0, index0 or index281279(one byte below required source index). Each refusal leaves source+visible display cache ownership intact at576000000tracked bytes, returns temporary index usage0, leaves store bytes/objects0 and preserves a recursive snapshot of temporary file paths. A RAM cache hit with all remaining root credit occupied shares prepared pixels without decoding after each refusal.

The same run then passes the existing successful segmented source/crop/cancellation/720pstreamed+cachedLOD/actualMetal qualification:32readbacks,final root0. The test-only matrix does not change quotas or storage behavior; it does not prove physical slow-disk throughput or preservation of unrelated evicted entries on failed replacement.

## Production ResidentSource integration on actual Metal

Extended the36MPtest into ResidentSource::install_with_prepared_lod/adopt_swapped/region_job/publish_region. A16MiBtexture quota forces a720x720aspect-preserving fit preview instead of the6000x6000texture; CPU viewport budget32MiB and upload queue16MiB. Fit1280x720half-float readback matches direct preview rendering. Stale identity adoption refuses; current identity adopts segmented storage, reports no remaining persistence source, and releases the576MBsource after cache/foreground owners drop, retaining only preview+index.

Before source release, three pan positions at zoom32 build packed references from original prepared pixels. After source file deletion, actual region_job rectangles restore from segments and publish via ResidentSource. Three full1280x720half-float readbacks exactly match references. Re-querying the contained region returns None. Dropping ResidentSource/renderer returns viewport/texture/upload credit0. The complete integration passes40readbacks (8newResidentSourcehalf-float checks plus prior32SDR/float checks), final root0,4.54seconds on M4 Max.

No production edits were needed for this integration; test-only extension. It exercises the production viewport component directly, not App event-loop/native window automation, physical HDR output or NVIDIA. Reference pixel/readback allocations remain external to tracked production-root accounting.

## Asynchronous ResidentSource/LodWorker publication

Replaced direct pan restore/publish calls in the36MPtest with production refresh_swapped/LodWorker submissions, completion wake-ups and publication. A test-only before_filter barrier confirms the first job is live while the caller remains free to render the installed fit preview. Advance the cancellation token epoch and request the opposite pan; refresh cancels the old revision and schedules the replacement. After releasing the barrier only the replacement completion wakes/publishes; its full720preadback matches the prepared-pixel reference. Three subsequent pan requests also use worker wake/refresh publication and match; contained re-queries schedule no extra completion.

Actual M4 Max integration passes4worker publications and10ResidentSourcehalf-float checks,42totalGPUreadbacks including prior crop/LOD paths,final viewport/texture/upload/root credit0. Worker shutdown occurs before final ownership assertions. This is controlled component scheduling, not native App event-loop/UI latency automation. Old job is blocked before filtering, while separate crop tests cover cancellation after admission. No production change in this follow-up.

## 36MP ICC public file/color working-row accounting

Added a second explicitly enabled public-file qualification: synthetic6000x6000PSD tiles the16x16pattern.profiled.pngnative RGB samples with its588byte ICC profile. The expected RGB comes from existing independently produced LittleCMS pattern.profiled.png.rgba, converted with a separate f64 sRGB formula; all36million RGB pixels agree within the established1/1024quantized-reference tolerance, alpha is exactly1. This is256repeated color samples, not36millionunique colors or allICCprofile classes.

Native retained credit144000588bytes (pixels+ICC), linear output576000000bytes, two f64RGB rows288000bytes produce exact tracked peak720288588bytes. Row credit releases after conversion; dropping native then prepared releases profile/native and output ownership in turn. Budget720288587refuses conversion and leaves only the original native owner; final0 after drop. RAII removes the synthetic file even on test failure. The large test passed in5.07seconds; default decoder regression also passes. Test-only extension, no new production change or new ICC36MPGPU/swap qualification.

## ICC36MP display cache / async viewport / actual Metal

Parameterized the existing large-file integration with a second ICC source. It tiles the same independently qualified16x16profiled pattern over6000x6000RGB pixels and embeds the pinned588byte profile. Production load_cached_raster_for_display verifies explicit ICC interpretation (assumed=false), native/color tracked peak720288588bytes, and576000000bytes retained prepared linear floats. The profile/native/working-row credit is released before prepared cache retention; segmented swap therefore stores the same linear payload shape576000064bytes without retaining ICC metadata unnecessarily.

Actual M4 Max ICC run passes all five disk/count/index refusals preserving visible RAM, three crop short-quota/cancel/retries, async ResidentSource generation/revision rejection and four pan publications, streamed/cached1280x720LOD and42exactGPUreadbacks. Source and full RAM release precede restoration; final tracked root0. Untagged branch of shared helper rerun separately also passes42readbacks. Full default viewer276 passed/76ignored; both manual hardware tests were separately enabled and passed. Test-only extension; physical HDR/native window/NVIDIA and allICCprofile classes remain unqualified.

## Large neighbour prefetch pressure and recovery

Each large-file variant now creates a second3000x3000PSD source (9MP) using the same explicit color policy. While the visible36MPprepared frame owns576MB, production preload_raster refuses the neighbour within the configured root limit. Refusal returns retained usage to576MB, and foreground visible cache access still shares the old pixels without decoding. The test does not require a second full frame to fit when the configured quota cannot admit it.

After current identity adopts segmented storage and the full visible RAM/cache owners drop, only the fit preview+index remain. The same neighbour request then preloads successfully into144000000byte/count1cache. Reserving all remaining root credit, a foreground next-file lookup shares those prepared pixels with a panic decoder fallback. Releasing neighbour/cache ownership returns usage to original preview+index; temporary sources are removed. Both ICC and assumed-sRGB variants pass this test plus their existing42actual M4 Max readbacks, final root0.

This directly qualifies production prefetch/cache calls with realistic source sizes, not native App navigation/event-loop scheduling or foreground replacement when two equal maximum-size frames cannot coexist. No production change was needed; test-only extension and no repeated default-suite run.


## ICC PCS D50 correction follow-up

Independent LittleCMS comparison found then fixed approximate-D50source/destination colorants. Both original RGB16profiles now meet unchanged2e-5RGB bound(max7.57e-6); current2014-case Metal and36MPICCasync chain pass after correction. Detailed evidence: [ICC PCS report](../icc-pcs-d50-correction-2026-10-16/REPORT.md). Earlier shared-transform transport checks remain distinct from this new independent accuracy evidence.
