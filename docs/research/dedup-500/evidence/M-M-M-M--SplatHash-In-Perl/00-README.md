# SplatHash in Perl

Perl implementation of [SplatHash](https://github.com/junevm/splathash), a
perceptual image placeholder format that encodes an image in exactly 16 bytes.

The implementation follows the Go reference at commit
`038b4f7025ec044ed9f319371e0ec2ed863a6f45` and includes bit-for-bit parity
tests on raw RGBA inputs.

## Installation

Required Perl modules:

- `Cwd` (core, for benchmarks)
- `Digest::SHA` (core, for asset integrity tests)
- `Exporter` (core)
- `ExtUtils::MakeMaker` (core, for `Makefile.PL`)
- `File::Path` (core, for benchmarks)
- `File::Spec` (core, for benchmarks)
- `JSON::PP` (core, for parity fixtures)
- `Getopt::Long` (core, for command-line tools)
- `HTTP::Tiny` (core, for benchmark asset downloads)
- `Test::More` (core, for tests)
- `File::Temp` (core, for tests)
- `Imager` (required for `encode_file`, the CLI, and image tests)
- `IPC::Open3` (core, for command tests)
- `Symbol` (core, for command tests)
- `Time::HiRes` (core, for benchmarks)

Development tools used by this repository:

- `perltidy`
- `perlcritic`
- `prove`
- `cpanm`

```sh
cpanm --installdeps .
cpanm Imager
perl Makefile.PL
make
make test
```

`encode_raw` and `decode` use only core Perl modules. `encode_file` requires
`Imager` because it loads image files before converting them to raw RGBA bytes.

## Usage

### Perl API

```perl
use SplatHash qw(encode_raw decode);

my $hash = encode_raw($rgba, $width, $height);
die 'unexpected hash length' if length($hash) != 16;

my $preview_rgba = decode($hash);
```

`$rgba` and `$preview_rgba` are row-major RGBA byte strings. Decoded previews
are always 32x32 pixels and therefore contain 4096 bytes.

### Command Line

```sh
perl -Ilib bin/splathash --input photo.jpg --output preview.png
```

The command prints the 16-byte hash as 32 lowercase hexadecimal characters and
writes `preview.png` as the decoded 32x32 PNG preview.

After installation, use:

```sh
splathash --input photo.jpg --output preview.png
```

### Benchmark

Compare Perl and the pinned Go reference on the same upstream asset images:

```sh
perl -Ilib bench/compare-perl-go.pl --format markdown
```

The repository includes the upstream `assets/` directory from commit
`038b4f7025ec044ed9f319371e0ec2ed863a6f45`. Checksums are stored in
`t/fixtures/upstream-assets.sha256`; see [ASSETS.md](ASSETS.md) for provenance.
To use a different image directory:

```sh
perl -Ilib bench/compare-perl-go.pl --assets path/to/images
```

By default, each image is encoded 10 times and decoded 1000 times by each
language. One untimed warm-up is run before every timed operation. Across the
9 bundled images, that means 90 timed encodes and 9000 timed decodes for Perl,
and the same counts for Go. Per image and language, the runner therefore calls
encode 12 times: once to prepare the decode hash, once for warm-up, and 10
timed times. Decode is called 1001 times: once for warm-up and 1000 timed
times. Use `--include-full` to additionally measure image loading plus
encoding.

#### Published Results

Measured on June 8, 2026 using an Apple M1 Max with 10 CPU cores and 64 GiB
RAM, macOS 26.5.1, Perl 5.42.2, and Go 1.26.4 darwin/arm64. The Go
implementation is pinned to commit
`038b4f7025ec044ed9f319371e0ec2ed863a6f45`. Results are environment-specific.

| Image | Perl encode (ms/op) | Go encode (ms/op) | Encode ratio | Perl decode (ms/op) | Go decode (ms/op) | Decode ratio |
|---|---:|---:|---:|---:|---:|---:|
| wallhaven-3q3j6y.jpg | 166.581 | **1.669** | 99.83x | 2.619 | **0.023** | 112.96x |
| wallhaven-8525yy.jpg | 166.999 | **1.654** | 100.96x | 2.667 | **0.020** | 132.79x |
| wallhaven-gw5dq3.jpg | 166.528 | **1.680** | 99.15x | 2.347 | **0.019** | 121.03x |
| wallhaven-gwpgv3.jpg | 166.829 | **1.657** | 100.65x | 2.440 | **0.018** | 134.04x |
| wallhaven-ogy2m7.jpg | 166.120 | **1.654** | 100.43x | 2.225 | **0.019** | 114.34x |
| wallhaven-ogy7ql.jpg | 168.675 | **1.649** | 102.26x | 2.502 | **0.022** | 115.34x |
| wallhaven-q2yx9l.jpg | 169.068 | **1.647** | 102.68x | 2.478 | **0.021** | 120.52x |
| wallhaven-w513lr.jpg | 167.170 | **1.663** | 100.55x | 2.405 | **0.021** | 117.11x |
| wallhaven-yq5ejd_3840x2160.png | 167.279 | **1.677** | 99.76x | 2.659 | **0.023** | 116.78x |
| **Average** | 167.250 | **1.661** | 100.69x | 2.482 | **0.021** | 120.19x |

The best time in each Perl/Go pair is shown in bold. The optimized pure Perl
encoder is about 4.8 times faster than the first published implementation, but
the remaining matching-pursuit loops are still roughly 100 times slower than
compiled Go. Matching Go performance will require a future compiled backend,
such as XS, while retaining the pure Perl implementation as a fallback.

See [DOCUMENTATION.md](DOCUMENTATION.md) for API and algorithm details.

## Development Provenance

This project was developed with assistance from OpenAI Codex using a
test-driven development workflow. Development plans and major decisions were
reviewed by a human maintainer, and automated tests and checks were run. The
source code has not received a comprehensive line-by-line human review.

The project also served as a practical validation case during preparation of
[perl-agents-md v1.0.0](https://github.com/M-M-M-M/perl-agents-md/releases/tag/v1.0.0).
This statement describes the development process; it is not a certification of
the project's quality or security.

## License

This Perl port is available under the [MIT License](LICENSE), copyright
`M-M-M-M`.

SplatHash incorporates material from the upstream SplatHash project. See the
[third-party licences](LICENSES/README.md) for the preserved upstream notice
and attribution.

## Development

```sh
prove -lr t
prove -lr xt
SPLATHASH_RUN_BENCH=1 SPLATHASH_ASSETS=assets prove -lr xt
perltidy -pro=.perltidyrc lib/*.pm t/*.t xt/*.t bin/splathash bench/compare-perl-go.pl
perlcritic lib t xt bin bench Makefile.PL
```

`prove -lr xt` runs author checks such as licence validation. The benchmark
author test is skipped unless `SPLATHASH_RUN_BENCH=1` is set.
