

### Current original CR3 Metal compute (2026-10-16)

Six Bayer algorithms executed on the full local CR3 sensor on Apple M4 Max Metal. Each compared 336 sampled points (including edges) against the independent f64 reconstruction reference, within the unchanged 32*EPSILON*max(abs(reference),1) bound. Maximum absolute error across all algorithms: 5.049571e-8. CPU/GPU tracked credits returned to zero after each algorithm. Source media and image readbacks were not exported. This is sampled reconstruction accuracy, not full-pixel equality, photographic color qualification, NVIDIA or physical HDR validation. Evidence: `research/current-real-bayer-metal-2026-10-16/metal.log`, `result.json`.


Full-pixel authored sensor follow-up: new retained ignored GPU test covers 2x2, 3x5, 8x7 and 17x19 sensors, all four Bayer phases, unequal green WB, nonlinear patterned sensor codes, and all six algorithms. All 96 executions / 9,552 pixels compared against the independent f64 reconstruction reference within unchanged bound; maximum absolute error 9.472769e-7, alpha exact, final CPU/GPU credits zero per execution. Actual adapter Metal Apple M4 Max. Evidence: `full-small-sensors.log`. This complements the original CR3 sampled test; it does not establish every original camera pixel or physical HDR behavior.


Calibration follow-up: full-pixel Bayer gate now additionally covers a 3x2 repeating spatial black-level grid and four distinct phase white levels, alongside scalar levels. The black-grid period differs from the Bayer period. All 192 executions / 19,104 pixels passed the unchanged f64 error bound on Metal M4 Max; maximum error 1.096391e-6, exact alpha, final CPU/GPU tracked credits zero per execution. Log: `full-calibrated-sensors.log`. This remains authored sensor calibration coverage, not arbitrary camera qualification.
