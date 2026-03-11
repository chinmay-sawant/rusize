mod chars;
mod cli;
mod display;
mod scanner;

use clap::Parser;
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::fs;
use std::path::PathBuf;
use sysinfo::{Disks, System};

use cli::Args;

fn main() -> anyhow::Result<()> {
    // Enable ANSI escape code processing on Windows cmd.exe / PowerShell.
    // Without this, cmd.exe shows raw codes like ←[1;96m instead of colors.
    // On non-Windows platforms this is a harmless no-op.
    enable_ansi_support::enable_ansi_support().ok();

    let args = Args::parse();
    let sys = System::new_all();
    let c = chars::get();
    let _fullscreen = display::enter_fullscreen()?;
    let min_bytes = (args.min_size * 1024.0 * 1024.0) as u64;

    // -- Banner & system info -----------------------------------------------
    display::banner(c);
    display::system_info(c);

    // -- Target discovery ---------------------------------------------------
    let targets: Vec<PathBuf> = if let Some(ref p) = args.path {
        vec![PathBuf::from(p)]
    } else {
        println!("\n{} {}", c.search, "Detecting system disks...".cyan());
        let disks = Disks::new_with_refreshed_list();
        disks
            .iter()
            .map(|d| d.mount_point().to_path_buf())
            .collect()
    };

    let cpu_count = sys.cpus().len();
    println!(
        "{} Found {} target(s). Using {} CPU threads.\n",
        c.folder,
        targets.len().to_string().green().bold(),
        cpu_count.to_string().yellow().bold()
    );

    // -- Scan each target ---------------------------------------------------
    let mut scan_targets = Vec::new();
    for root in &targets {
        display::scan_header(&root.display().to_string(), c);

        // Collect top-level directories
        let top_dirs: Vec<PathBuf> = match fs::read_dir(root) {
            Ok(rd) => rd
                .filter_map(|res| res.ok().map(|e| e.path()))
                .filter(|p| p.is_dir())
                .collect(),
            Err(e) => {
                eprintln!(
                    "  {} Could not read {}: {}",
                    c.cross.red().bold(),
                    root.display(),
                    e
                );
                continue;
            }
        };

        // Progress bar
        let bar_chars = if cfg!(target_os = "windows") || std::env::var("RUSIZE_ASCII").is_ok() {
            "##-"
        } else {
            "█▓░"
        };
        let pb = ProgressBar::new(top_dirs.len() as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("  {spinner:.green} [{bar:40.cyan/dim}] {pos}/{len} folders ({eta})")
                .unwrap()
                .progress_chars(bar_chars),
        );

        // Parallel tree scan via Rayon
        let depth = args.depth;
        let mut nodes: Vec<scanner::DirNode> = top_dirs
            .into_par_iter()
            .map(|path| {
                let node = scanner::scan_tree(&path, 1, depth);
                pb.inc(1);
                node
            })
            .collect();

        pb.finish_and_clear();

        // Sort (recursively) if requested
        if args.sort {
            scanner::sort_recursive(&mut nodes);
        }

        let total_size = nodes.iter().map(|node| node.size).sum();
        scan_targets.push(display::ScanTarget {
            root_display: root.display().to_string(),
            nodes,
            total_size,
        });
    }

    display::run_app(scan_targets, min_bytes, args.sort, args.depth, c)?;

    Ok(())
}
