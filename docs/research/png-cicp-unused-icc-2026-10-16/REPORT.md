# Skip PNG ICC work only when validated cICP already overrides it

The native PNG declaration parser already gives cICP priority over iCCP. Nevertheless, the still-raster path used to let the codec inflate/store ICC and then request a cloned profile that it discarded during color selection. A new explicit vendored constructor skips text and ICC metadata only when the application has already parsed a valid higher-priority cICP declaration. The profile getter is also skipped in this case. Existing constructors and PNG sources without cICP keep their previous ICC behavior. EXIF and checksum policy remain unchanged.

On a reproducibly generated16x16 PNG with cICP linear-sRGB and a4MiB padded ICC profile, the same-library test-only allocator comparison measures codec construction+decode with and without ICC collection, excluding caller input/output: **4372728 requested heap bytes /23 allocations →27768 bytes /6 allocations**. All pixels exactly match the pinned oracle and all measured private allocations release. This probe does not include the additional unused cloned ICC getter from the old application route, and does not claim a whole-process RSS or latency result.

Public regression covers normal588-byte and padded4MiB ICC data with no cICP, sRGB cICP and linear-sRGB cICP (6controls). Profile-only sources retain every ICC byte; cICP sources retain their declared color interpretation; all pixel bytes match the original oracle. Corrupt ICC CRC remains rejected by native declaration parsing even when ICC decoding is skipped. Managed pixel credit returns to0 after success and error. Full public decoder library: **202 passed /5 ignored /zero failures**.

The padded profile is based on the pinned valid profile, retaining its tags and updating its declared total length; padding is synthetic. No arbitrary original camera/cICP/ICC variant qualification is inferred. Full shared-root accounting of decoder state, retained ICC metadata when used, EXIF and row buffers remains open. APNG is not switched by this still-raster routing change. No physical HDR/NVIDIA or complete100-format claim.

Viewer gate: **285 passed /80 ignored /zero failures** (`viewer.log`).
