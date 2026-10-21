# Metal collection soak — failed RSS guard

The current frozen binary requested 600 seconds of eight 4000x3000 RGBA32Float frames with full SDR and 4000x3000 binary16 HDR readback on actual Metal. It was terminated by the owned monitor after three consecutive RSS samples exceeded the unchanged 4 GiB guard. Exit -15 is deliberate monitor termination, not a test assertion failure or completed qualification.

Samples rise from 1814790144 bytes at 5.008 seconds to 7548518400 at 25.057 seconds. 56 cycles completed exact frame assertions before termination; their tracked root/queue credits return to zero. This does not prove release of driver/GPU/test-harness allocations. No 600-second pass is claimed. The frozen binary hash and source hashes are retained in state.json and sources.json; raw test and RSS logs are preserved.

Candidate causes remain unproven: retained backend objects, deferred native resource release, repeated renderer/pipeline construction, or readback allocation lifetime. Expected reference storage alone is fixed (eight SDR plus eight HDR frames), so it cannot by itself explain continuing growth. Next action is controlled resource-lifetime isolation with the same full-frame workload and guard, preserving this failed run. The guard must not simply be raised to obtain a pass. No GUI window was launched; physical HDR and NVIDIA remain unqualified.
