# [image-find](https://github.com/ImShyMike/image-find)

A high-performance tool to find similar images using [perceptual hashing](https://en.wikipedia.org/wiki/Perceptual_hashing).

![Demo GIF](https://raw.githubusercontent.com/ImShyMike/image-find/refs/heads/main/assets/demo.gif)

---

## Table of contents

* [Installation](#installation)
  * [From crates.io](#from-cratesio)
  * [Building from source](#building-from-source)
* [Usage](#usage)
* [Output](#output)
  * [CSV Report](#csv-report)
* [Demo GIF](#demo-gif)
* [License](#license)

## Installation

### From crates.io

```bash
cargo install image-find
```

### Building from source

```bash
git clone https://github.com/imshymike/image-find.git
cd image-find
cargo install --path .
```

## Usage

```bash
$ image-find
Usage: image-find <wanted_images_folder> <search_folder> [<similarity_threshold>] [<found_matches_folder>]
```

> [!NOTE]
> The default similarity threshold is 90%.

The program analyzes all images in the `search_folder` directory and compares them against reference images stored in the `wanted_images_folder`. When matches are found that exceed the specified `similarity_threshold`, they are printed to the console. Optionally, you can provide a `found_matches_folder` path to automatically copy all discovered matches to a separate location.

| Arguments              | Type       | Description                                   | Example value   |
|------------------------|------------|-----------------------------------------------|-----------------|
| `wanted_images_folder` | Path       | Directory containing reference images to find | `wanted_images` |
| `search_folder`        | Path       | Directory to search for similar images        | `all_images`    |
| `similarity_threshold` | Percentage | Minimum similarity score (0-100%)             | `90`            |
| `found_matches_folder` | Path       | Optional output directory for matched images  | `found`         |

## Output

### CSV Report

A CSV file is generated in the directory where the program was executed containing all found matches with the following format:

| Column       | Description                                     | Example           |
|--------------|-------------------------------------------------|-------------------|
| `id`         | Filename of the matched image with no extension | `200`             |
| `file`       | Full filename of the matched image              | `200.jpg`         |
| `path`       | Full path to the matched image                  | `/images/200.jpg` |
| `similarity` | Similarity score (0.0-1.0, 4 decimal places)    | `0.9700`          |

Example CSV output:

```csv
id,file,path,similarity
200,200.jpg,~/images/200.jpg,0.9700
341,341.png,~/images/341.jpg,1.0000
```

## Demo GIF

The demo GIF is made using [vhs](https://github.com/charmbracelet/vhs) with the command `vhs cassette.tape`.

## License

This project is licensed under the [MIT license](./LICENSE).
