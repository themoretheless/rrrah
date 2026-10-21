# EPS source lifetime before raster admission

The owned file source previously overlapped pixel working buffers, although compilation and VM evaluation had already been released. Preparation now returns an owned vector scene; the file entry releases source credit before rasterization. The borrowed-byte entry retains its caller-owned source contract.

Regression: a 512x512 solid-red EPS with 4096 comment padding lines. The borrowed-input managed peak is 1,313,240 bytes (caller input excluded). Before the change the file entry failed at that same root limit: requested 1,048,576 bytes, used 276,584. See red.log. The new regression requires exact red pixels, success within that calibrated budget, output-only retained credit and zero credit after teardown.

This is managed admission evidence, not an RSS or latency benchmark. Source limits, image-index refusal and request cancellation remain enforced.

Validation: rrrah-vector library 252 passed / 11 ignored / 0 failed; required Metal EPS lifecycle 15 passed / 0 ignored / 0 failed. JSON reports/fixtures parsed (161 files) and both modified Python scripts passed syntax compilation; source/documentation whitespace check passed. Original research logs and diagnostic patch bytes are preserved, including their whitespace.
