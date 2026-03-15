# rusize

**rusize** is a blazing-fast, multi-threaded disk space analyzer written in Rust. 
It uses Rayon's work-stealing thread pool to scan directories in parallel, detects system disks automatically, and outputs results in customizable reporting formats (CSV, JSON, Text).

## Features

- **Blazing Fast**: Uses multi-threading (`rayon`) to scan directories in parallel.
- **Auto-Detection**: Automatically detects target disks if no path is specified.
- **Reporting Formats**: Output data to CSV (default), JSON, or a Text tree-based layout.
- **Cross-Platform**: Works correctly on Windows, macOS, and Linux.
- **Filtering**: Easily filter folders by minimum size using the `--min-size` argument.

## Installation / Building

Make sure you have [Rust](https://www.rust-lang.org/) installed, then run:

```sh
cargo build --release
```

The executable will be available at `target/release/rusize`.

### Windows Build from Linux

If you are on a Linux system and want to cross-compile for Windows:

```sh
# Add the Windows GNU target
rustup target add x86_64-pc-windows-gnu

# Install the MinGW-w64 toolchain (example for Debian/Ubuntu)
sudo apt install mingw-w64

# Build for Windows
cargo build --release --target x86_64-pc-windows-gnu
```

## Usage

You can run `rusize` directly. By default, it will auto-detect system disks, show folders larger than 1 MB, and scan up to 1 level deep.

```sh
rusize [OPTIONS] [PATH]
```

### Arguments & Options

- `[PATH]`: Optional directory path to scan. If omitted, `rusize` automatically discovers and scans all system disks.
- `-m, --min-size <MB>`: The minimum directory size to display, in megabytes (default: `500.0`).
- `--no-sort`: Disable sorting of directories by size (they are sorted largest-to-smallest by default).
- `-d, --depth <LEVELS>`: Depth of the directory tree to scan (default: `10`).
- `-f, --format <FORMAT>`: Output format for the report. Options are `csv`, `json`, `text` (default: `csv`).
- `-o, --output <OUTPUT_PATH>`: Path to save the report to. If omitted, reports are saved to `rusize_report.<format>`.
- `-h, --help`: Print help information.
- `-V, --version`: Print version information.

### Examples

Scan the current directory, generating a CSV report (default):
```sh
rusize .
```

Scan a specific directory showing only folders larger than 50 MB, formatted as an ASCII tree:
```sh
rusize --format text --min-size 50 /var/log
```

Export scan output to a JSON file:
```sh
rusize --format json . > output.json
```

## License

Created by **Chinmay Sawant**.
