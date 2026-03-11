use colored::*;
use std::io::{self, Write};

use crate::chars::Chars;
use crate::scanner::DirNode;

// ---------------------------------------------------------------------------
// Banner
// ---------------------------------------------------------------------------

/// Print the application banner.
pub fn banner(c: &Chars) {
    let hz_line: String = c.hz.repeat(50);

    println!();
    println!(
        "{}",
        format!("{}{}{}", c.tl, hz_line, c.tr).bright_cyan().bold()
    );
    println!(
        "{}",
        format!(
            "{}  {} rusize -- Disk Scanner                       {}",
            c.vt, c.bolt, c.vt
        )
        .bright_cyan()
        .bold()
    );
    println!(
        "{}",
        format!(
            "{}  High-Speed  |  Multi-Threaded  |  Tree View     {}",
            c.vt, c.vt
        )
        .bright_cyan()
        .bold()
    );
    println!(
        "{}",
        format!("{}{}{}", c.bl, hz_line, c.br).bright_cyan().bold()
    );
}

// ---------------------------------------------------------------------------
// System info
// ---------------------------------------------------------------------------

/// Print OS and privilege status.
pub fn system_info(c: &Chars) {
    let os = std::env::consts::OS;
    let is_admin = is_root::is_root();

    let status = if is_admin {
        "Yes".green().bold()
    } else {
        "No".red().bold()
    };

    println!(
        "\n{}  System: {}  |  Elevated: {}",
        c.system,
        os.bright_magenta().bold(),
        status
    );

    if !is_admin {
        println!(
            "{}",
            format!(
                "{}  Running without Sudo/Admin. Some folders may be skipped.",
                c.warn
            )
            .yellow()
            .dimmed()
        );
    }
}

// ---------------------------------------------------------------------------
// Tree printer
// ---------------------------------------------------------------------------

/// Print a single scan root header.
pub fn scan_header(root_display: &str, c: &Chars) {
    println!(
        "{}  {}",
        c.arrow.bright_white().bold(),
        format!("Scanning: {}", root_display).bright_cyan()
    );
}

/// Print the directory tree starting from a list of top-level nodes.
pub fn tree(nodes: &[DirNode], min_bytes: u64, sort: bool, c: &Chars) {
    let visible: Vec<&DirNode> = nodes.iter().filter(|n| n.size >= min_bytes).collect();

    if visible.is_empty() {
        let min_mb = min_bytes as f64 / 1024.0 / 1024.0;
        println!(
            "  {} No folders above {:.1} MB threshold.",
            c.info.yellow(),
            min_mb
        );
        return;
    }

    println!();
    let count = visible.len();
    for (i, node) in visible.iter().enumerate() {
        let is_last = i == count - 1;
        print_node(node, "  ", is_last, min_bytes, sort, c);
    }
}

/// Recursively print a single tree node and its children.
fn print_node(node: &DirNode, prefix: &str, is_last: bool, min_bytes: u64, sort: bool, c: &Chars) {
    let connector = if is_last { c.last_branch } else { c.branch };
    let size_mb = node.size as f64 / 1024.0 / 1024.0;

    println!(
        "{}{}{:<45} {:>12}",
        prefix,
        connector.bright_black(),
        node.name.blue(),
        format_size(size_mb).bright_white().bold()
    );

    let child_prefix = format!("{}{}", prefix, if is_last { "    " } else { c.vertical });

    let mut children: Vec<&DirNode> = node
        .children
        .iter()
        .filter(|n| n.size >= min_bytes)
        .collect();

    if sort {
        children.sort_by(|a, b| b.size.cmp(&a.size));
    }

    let child_count = children.len();
    for (i, child) in children.iter().enumerate() {
        print_node(
            child,
            &child_prefix,
            i == child_count - 1,
            min_bytes,
            sort,
            c,
        );
    }
}

// ---------------------------------------------------------------------------
// Totals
// ---------------------------------------------------------------------------

/// Print the total scanned size for a root target.
pub fn total(nodes: &[DirNode], c: &Chars) {
    let total_bytes: u64 = nodes.iter().map(|n| n.size).sum();
    let total_mb = total_bytes as f64 / 1024.0 / 1024.0;
    println!(
        "\n  {} Total scanned: {}\n",
        c.sigma.bright_magenta().bold(),
        format_size(total_mb).green().bold()
    );
}

// ---------------------------------------------------------------------------
// Bar chart
// ---------------------------------------------------------------------------

/// Print a horizontal bar chart and percentage breakdown for the given nodes.
pub fn bar_chart(nodes: &[DirNode], min_bytes: u64, c: &Chars) {
    let visible: Vec<&DirNode> = nodes.iter().filter(|n| n.size >= min_bytes).collect();
    if visible.is_empty() {
        return;
    }

    let max_size = visible.iter().map(|n| n.size).max().unwrap_or(1);
    let bar_width: usize = 35;

    println!(
        "  {} {}",
        c.chart,
        "Size Distribution".bright_white().bold().underline()
    );
    println!();

    for node in &visible {
        let ratio = node.size as f64 / max_size as f64;
        let filled = (ratio * bar_width as f64) as usize;
        let empty = bar_width - filled;

        let bar = format!("{}{}", c.bar_full.repeat(filled), c.bar_empty.repeat(empty));
        let size_mb = node.size as f64 / 1024.0 / 1024.0;

        println!(
            "  {:<30} [{}] {}",
            node.name.blue(),
            bar.cyan(),
            format_size(size_mb).bright_white()
        );
    }

    // Percentage breakdown
    let grand_total: u64 = visible.iter().map(|n| n.size).sum();
    if grand_total > 0 {
        println!(
            "\n  {} {}",
            c.chart,
            "Percentage Breakdown".bright_white().bold().underline()
        );
        println!();

        for node in &visible {
            let pct = node.size as f64 / grand_total as f64 * 100.0;
            let size_mb = node.size as f64 / 1024.0 / 1024.0;
            println!(
                "  {:<30} {:>6.1}%   ({})",
                node.name.blue(),
                pct,
                format_size(size_mb).dimmed()
            );
        }
    }

    println!();
}

// ---------------------------------------------------------------------------
// Completion
// ---------------------------------------------------------------------------

/// Print the "scan complete" message.
pub fn done(c: &Chars) {
    println!("{}", format!("{} Scan complete.", c.check).green().bold());
}

/// Block until the user presses Enter.
///
/// This keeps the terminal window open when `rusize.exe` is launched via
/// double-click on Windows.
pub fn wait_for_exit() {
    println!();
    print!("{}", "Press Enter to exit...".dimmed());
    io::stdout().flush().ok();
    let mut buf = String::new();
    io::stdin().read_line(&mut buf).ok();
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Format a size in megabytes into a human-readable string (MB / GB / TB).
pub fn format_size(size_mb: f64) -> String {
    if size_mb >= 1_048_576.0 {
        format!("{:.2} TB", size_mb / 1_048_576.0)
    } else if size_mb >= 1024.0 {
        format!("{:.2} GB", size_mb / 1024.0)
    } else {
        format!("{:.2} MB", size_mb)
    }
}
