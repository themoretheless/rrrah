# Interrupt PNG input reads for obsolete generations

The virtual PNG source now retains the request cancellation token, checks it on every fill_buf call and exposes at most65536contiguous compressed input bytes. A cancelled read returns Other(Cancelled), not Interrupted, so read_exact cannot endlessly retry a permanently cancelled source. Logical read/seek/EXIF exclusion semantics remain unchanged.

PNG constructor and generic pixel-decode errors now recheck the request token: an active request keeps its original codec error, while an obsolete request returns typed Source(Cancelled). This prevents cancellation I/O from being misreported as corrupted image data. A successful decode still receives the existing later generation/publication checks.

Controls verify source chunk bounds, cancellation without advancing stream position, and the non-retry error kind. A valid pinned PNG is prepared and its decoder constructed while generation1 is active. After1024bytes of output credit is admitted, generation advances to2; DynamicImage decoding reaches the cancelled source and returns typed Cancelled, without a frame. Output admission is released. A new token for generation2 retries successfully, produces every exact oracle pixel, and releases admission to0. The helper deliberately does not precheck the token before calling the codec, so this control exercises the read interruption rather than merely an early cancellation guard.

Full public decode library: **216 passed /5 ignored /zero failures**. Viewer: **285 passed /80 ignored /zero failures**. Existing metadata/read/seek/ICC/EXIF/color/pixel/budget cases remain green. Logs and current source hashes retained.

65536is the maximum exposed compressed input slice, not a proven wall-clock cancellation deadline. Codec work between source calls, fixed PNG tables/row workspace, APNG and other codecs remain separate concerns. No whole-viewer latency, NVIDIA/CUDA or complete format qualification claim.
