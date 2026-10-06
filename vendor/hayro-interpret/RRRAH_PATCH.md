Registry upstream hayro-interpret; see preserved Cargo.toml version and LICENSE files.

Local DeviceCMYK precision work: explicit opt-in original-stage hybrid interpolation in moxcms; float DeviceCMYK fills bypass pre-ICC rounding in hayro-interpret. Independent qualification and limitations are recorded in docs/GOAL_COVERAGE.md.

DeviceCMYK8 images normalize and transform in blocks of at most 256 pixels using stack CMYK/RGB scratch, with paired input/output extent validation.
