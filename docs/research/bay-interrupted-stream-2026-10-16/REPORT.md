# BAY interrupted stream recovery

Native explicit-layout BAY row reading now checks cancellation before each partial read and retries Interrupted errors, including the trailing EOF check. Previously read_exact could retry interruptions without consulting the cancellation token; the final read treated Interrupted as a terminal I/O error.

Regression covers QV2000UX, QV3000EX and QV5700 image samples: cancellation on the third interrupted read returns typed cancellation with no retained credit; same-budget retry reads in 17-byte pieces, retries one EOF interruption, and matches every row against the existing unpack oracle. Exact output-plus-row peak and final zero are asserted. This is authored I/O regression evidence, not independent camera-file or color qualification. BAY remains Pending for CFA/WB/color/viewer routing.

Focused results: 8 passed / 2 ignored. Full raw-library result is retained in suite.log.
