blockhash [![Hackage](https://img.shields.io/hackage/v/blockhash.svg?style=flat)](https://hackage.haskell.org/package/blockhash)
=========

This is a perceptual image hash calculation tool based on algorithm described in
Block Mean Value Based Image Perceptual Hashing by Bian Yang, Fan Gu and Xiamu Niu.
Visit [the website][blockhash] for further information.

[blockhash]: http://blockhash.io/

## Installation

### From Hackage

```
cabal install blockhash
```

### From source

```
stack build
```

## Program

```
Usage: blockhash [-q|--quick] [-b|--bits ARG] filenames
  blockhash

Available options:
  -h,--help                Show this help text
  -q,--quick               Use quick hashing method
  -b,--bits ARG            Create hash of size N^2 bits.
```

## Library

The library exposes the `Data.Blockhash` module with the following API:

- `blockhash` - Calculate perceptual hash for an RGBA image
- `hammingDistance` - Calculate the hamming distance between two hashes
- `Image` - Image data type (width, height, RGBA pixels)
- `Hash` - Hash result type
- `Method` - Hashing method (`Precise` or `Quick`)

The example code below uses [JuicyPixels][JuicyPixels] to load images and prints
the hash to stdout.

```haskell
import qualified Codec.Picture as P
import Data.Blockhash
import qualified Data.Vector.Generic as VG
import qualified Data.Vector.Unboxed as V

printHash :: FilePath -> IO ()
printHash filename = do
  res <- P.readImage filename
  case res of
    Left err -> putStrLn ("Fail to read: " ++ filename)
    Right dynamicImage -> do
      let rgbaImage = P.convertRGBA8 dynamicImage
          pixels = VG.convert (P.imageData rgbaImage)
          image = Image { imagePixels = pixels
                        , imageWidth = P.imageWidth rgbaImage
                        , imageHeight = P.imageHeight rgbaImage }
          hash = blockhash image 16 Precise
      putStrLn (show hash)
```

[JuicyPixels]: https://hackage.haskell.org/package/JuicyPixels
