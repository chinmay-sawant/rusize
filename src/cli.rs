use clap::Parser;

/// CLI argument definitions for rusize.
///
/// All arguments are optional — by default rusize auto-detects disks,
/// shows folders > 1 MB, and scans 1 level deep.
#[derive(Parser, Debug)]
#[command(
    name = "rusize",
    author,
    version,
    about = "rusize -- High-speed Multi-threaded Disk Scanner",
    long_about = "\
rusize is a blazing-fast, multi-threaded disk space analyzer.\n\
It uses Rayon's work-stealing thread pool to scan directories in parallel,\n\
detects system disks automatically, and displays results as a tree with\n\
an ASCII bar chart.\n\
\n\
On Windows, it works correctly in cmd.exe, PowerShell, and Windows Terminal."
)]
pub struct Args {
    pub path: Option<String>,
    pub min_size: f64,
    pub sort: bool,
    pub depth: usize,
}
