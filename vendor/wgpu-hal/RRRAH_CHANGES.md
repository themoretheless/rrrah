Upstream wgpu-hal30.0.0 Cargo registry source; licenses retained.

Metal: remember native command-buffer Error as sticky device loss, inspect every submitted buffer, wake fence waiters on errors, and refuse successful waits/polls after failed execution. This addresses false success observed during a real GPU hang/reset; it does not repair or diagnose the cause of the hang.
