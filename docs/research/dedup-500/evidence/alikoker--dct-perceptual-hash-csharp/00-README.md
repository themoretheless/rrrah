# DCT Perceptual Hash for C#

## Overview

This repository publishes Muhammet Ali Köker's C# implementation of a compact 64-bit perceptual image hash based on a global discrete cosine transform (DCT), low-frequency coefficient selection, median quantization, and SIMD-assisted dot products.

The implementation was developed for practical image-similarity work rather than cryptographic integrity checking. According to the author, it has been exercised successfully in institutional live workflows and tested against large image datasets. The accompanying technical article also documents successful live use in an institutional matching problem. No synthetic benchmark ranking is published here because a reproducible benchmark record was not part of this release.

The core algorithm is preserved. Publication changes are limited to a corrected malformed source comment, clearer private constant/method names, explicit input validation for the existing 64 x 64 calibration precondition, a file-based helper overload, repository infrastructure, tests, examples, and a command-line application.

## Algorithm

For each image, the implementation:

1. maps the source into a fixed 64 x 64 working bitmap while preserving the original aspect-ratio behavior;
2. derives channel calibration values from the compatibility 64 x 64 region used by the original implementation;
3. reduces RGB values to a single intensity sequence;
4. applies a separable DCT, with cosine coefficients precomputed once;
5. uses `Vector4.Dot` in the transform hot path;
6. retains the upper-left 8 x 8 low-frequency coefficient region;
7. computes the median of the 64 selected coefficients;
8. emits one bit per coefficient according to its position relative to the median.

The result is a `UInt64`. Similarity between two hashes can be evaluated cheaply with Hamming distance (`popcount(a XOR b)`), but an application-specific threshold must be validated for the target image domain.

This is a perceptual hash, not a cryptographic hash. Collisions and near-collisions are expected properties of this representation.

## Operational provenance

This implementation is not presented as an untested demonstration. The author reports that the design has been used successfully in institutional image-matching workflows and evaluated with large datasets. The related technical article explains why the eight-byte representation is attractive for large collections: the expensive image decoding and DCT stage is performed once, while subsequent candidate comparisons can operate on compact 64-bit values.

The repository deliberately does not claim universal performance, a fixed accuracy rate, or a state-of-the-art ranking. Those claims would require a published dataset, hardware profile, threshold policy, and reproducible benchmark methodology.

## Library usage

```csharp
using MakLib.Imaging;

ulong hash = PerceptualHash.Compute(@"C:\images\sample.png");

Console.WriteLine(hash.ToString("X16"));
Console.WriteLine(hash);
```

The original bitmap API remains available:

```csharp
using System.Drawing;
using MakLib.Imaging;

using (Bitmap bitmap = new Bitmap(@"C:\images\sample.jpg"))
{
    ulong hash = PerceptualHash.Compute(bitmap);
}
```

## Command-line application

The `phash` console application accepts one or more file paths through `string[] args`:

```text
phash image1.jpg image2.png image3.tiff
```

Output is intentionally checksum-like and line-oriented:

```text
<HEX64>  <DECIMAL64>  <PATH>
```

For example:

```text
9A3F0C12D48761BE  11114614820587979198  image1.jpg
```

The hexadecimal field is fixed-width uppercase 16-digit output for the complete 64-bit value. The decimal field is the same unsigned value represented in base 10. The path is retained as the final field so a caller can process multiple files without losing input identity.

This format is inspired by common checksum utilities in placing the digest first and the file name last, but the additional decimal column means it is not intended to be byte-for-byte compatible with `md5sum` or `sha*sum` parsers.

The process continues after an individual file fails. Successful processing returns exit code `0`; one or more per-file failures return `1`; missing command-line arguments return `2`. Errors are written to standard error.

## Supported files

The compatibility CLI accepts:

- `.bmp`
- `.gif`
- `.jpg`
- `.jpeg`
- `.png`
- `.tif`
- `.tiff`

This list follows the bitmap formats documented for the `System.Drawing` / GDI+ file-loading path used by the original implementation: BMP, GIF, JPEG, PNG, and TIFF.

Extension checking is only the first gate. A file must still be a valid image that the underlying decoder can open.

Microsoft documentation:
https://learn.microsoft.com/en-us/dotnet/api/system.drawing.bitmap.-ctor

## Build

The projects target .NET Framework 4.8 to preserve the original `System.Drawing` and `System.Numerics.Vector4` implementation without introducing an image-processing dependency.

Build the CLI on a Windows development environment with a suitable Visual Studio/MSBuild toolchain:

```text
msbuild cli\PerceptualHash.Cli\PerceptualHash.Cli.csproj /p:Configuration=Release
```

Build the deterministic test harness similarly:

```text
msbuild tests\DctPerceptualHash.Tests\DctPerceptualHash.Tests.csproj /p:Configuration=Release
```

## Compatibility behavior and limitations

The original implementation calibrates channel ranges from a 64 x 64 region of the source bitmap while the DCT operates on the resized 64 x 64 working bitmap. That behavior is preserved because changing it would change established hash values.

As a consequence, source bitmaps smaller than 64 x 64 were outside the effective input contract of the submitted implementation. This release makes that precondition explicit and fails with `ArgumentException` instead of allowing an indirect pixel-access failure.

The global DCT representation is intended to tolerate changes such as rescaling, recompression, limited blur/noise, and moderate brightness/contrast variation. It is not inherently invariant to large crops, rotation, mirroring, perspective changes, or major object movement.

Do not use this value for integrity verification, authentication, signatures, password storage, or any other cryptographic purpose.

## Example files

`examples/sample-gradient.png` and `examples/sample-gradient.bmp` contain the same synthetic 96 x 96 RGB image in two lossless formats. They can be passed directly to the CLI and contain no production or private data.

## Testing

The repository includes a dependency-free test executable covering:

- supported-extension matching;
- case-insensitive extension handling;
- rejection of unsupported extensions;
- deterministic repeat computation on the same bitmap;
- explicit rejection of images smaller than the preserved 64 x 64 compatibility precondition.

No third-party test framework is required.

## Technical background

The implementation rationale, DCT structure, SIMD coefficient evaluation, median quantization, Hamming-distance comparison, institutional-scale motivation, successful live-use context, and known limitations are discussed in:

[Perceptual Image Similarity with DCT and SIMD](https://alikoker.com.tr/en/perceptual-image-similarity-with-dct-and-simd)

<!-- alikoker-research-metadata:start -->
## Research Software Record

[![DOI](https://zenodo.org/badge/DOI/10.5281/zenodo.22117445.svg)](https://doi.org/10.5281/zenodo.22117445)

- Archived software release (Zenodo DOI): [10.5281/zenodo.22117445](https://doi.org/10.5281/zenodo.22117445)
- Source repository: [https://github.com/alikoker/dct-perceptual-hash-csharp](https://github.com/alikoker/dct-perceptual-hash-csharp)
- Technical article (English): [https://alikoker.com.tr/en/perceptual-image-similarity-with-dct-and-simd](https://alikoker.com.tr/en/perceptual-image-similarity-with-dct-and-simd)
- Teknik makale (Türkçe): [https://alikoker.com.tr/dct-ve-simd-ile-algisal-goruntu-benzerligi](https://alikoker.com.tr/dct-ve-simd-ile-algisal-goruntu-benzerligi)
- ORCID: [0000-0003-3183-8378](https://orcid.org/0000-0003-3183-8378)
<!-- alikoker-research-metadata:end -->

## Citation

Software citation metadata is provided in `CITATION.cff`. The technical article above is the preferred contextual reference for the design and engineering background of this implementation.

## License

Apache License 2.0. See `LICENSE`.

## Author

Muhammet Ali Köker  
https://alikoker.com.tr/
