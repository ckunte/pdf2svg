# pdf2svg

A small command-line tool that converts PDF pages into standalone SVG files, built on [hayro-svg] (part of the [hayro] PDF engine). Text stays as real vector glyphs, vector art stays vector, and embedded raster images are carried over as embedded images — no rasterisation step involved.

[hayro-svg]: https://github.com/LaurenzV/hayro/tree/main/hayro-svg
[hayro]: https://github.com/LaurenzV/hayro

## Build

Requires a recent Rust toolchain (2021 edition, tested with 1.95).

```sh
RUSTFLAGS="-C target-cpu=native" cargo build --release
```

The binary is written to `target/release/pdf2svg`.

## Usage

```sh
# Convert every page of report.pdf into report-1.svg, report-2.svg, ...
# in the same directory as report.pdf
pdf2svg report.pdf

# Write into a specific directory
pdf2svg report.pdf -o out/

# Only convert pages 1, 3, and 5 through 8
pdf2svg report.pdf --pages 1,3,5-8

# A single selected page is written as "<prefix>.svg" (no page suffix)
pdf2svg report.pdf --pages 2 --prefix cover

# Encrypted PDF
pdf2svg report.pdf --password "secret"

# Give the SVG root element an opaque background instead of transparent
pdf2svg report.pdf --background '#ffffff'

# Overwrite existing output files, and print progress
pdf2svg report.pdf --force --verbose
```

Run `pdf2svg --help` for the full option list.

## Notes

- Pages are numbered starting at 1 in `--pages` and in the default output filenames
- By default, output files are refused if they already already exist; pass `-f`/`--force` to overwrite
- `--no-annotations` skips rendering form fields and other annotations
- See the [hayro] project for the supported PDF feature set and known limitations of the underlying engine
