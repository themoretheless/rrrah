# Explicit readback destruction — rejected diagnostic

Explicitly destroying the completed SDR/HDR target textures and readback buffers after unmapping did not resolve RSS growth. The monitor terminated this 60-second pilot at its unchanged 4 GiB guard. The original common harness source has been restored. `rejected-diagnostic.patch` retains the change; raw RSS/state/test logs and a live vmmap summary are retained.

At the live vmmap observation, MALLOC_LARGE (empty) accounts for 2.5 GiB resident, while owned unmapped graphics is about 618 MiB. This points toward allocator retention as a substantial contributor, but does not by itself prove the absence of other retained allocations. No production change is accepted.
