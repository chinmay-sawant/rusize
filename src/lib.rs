pub mod models;
pub mod services;
pub mod utils;

pub use services::report::ReportFormat;
use services::report::generate_report;
use services::scanner::{scan_tree, sort_recursive};
use std::path::PathBuf;
use sysinfo::Disks;
use rayon::prelude::*;

pub fn run(
    path: Option<String>,
    min_size_mb: f64,
    sort: bool,
    max_depth: usize,
    format: ReportFormat,
    output: Option<String>,
) -> anyhow::Result<()> {
    let min_bytes = (min_size_mb * 1024.0 * 1024.0) as u64;

    let targets: Vec<PathBuf> = if let Some(ref p) = path {
        vec![PathBuf::from(p)]
    } else {
        println!("Detecting system disks...");
        let disks = Disks::new_with_refreshed_list();
        disks
            .iter()
            .map(|d| d.mount_point().to_path_buf())
            .collect()
    };

    let mut all_nodes = Vec::new();

    for root in &targets {
        let top_dirs: Vec<PathBuf> = match std::fs::read_dir(root) {
            Ok(rd) => rd
                .filter_map(|res| res.ok().map(|e| e.path()))
                .filter(|p| p.is_dir())
                .collect(),
            Err(e) => {
                println!("Could not read {}: {}", root.display(), e);
                continue;
            }
        };

        println!("Scanning {}...", root.display());

        let mut nodes: Vec<models::dir_node::DirNode> = top_dirs
            .into_par_iter()
            .map(|p| scan_tree(&p, 1, max_depth))
            .collect();

        if sort {
            sort_recursive(&mut nodes);
        }

        // Apply min_size recursively by filtering out children under min_bytes
        fn retain_large(nodes: &mut Vec<models::dir_node::DirNode>, min_bytes: u64) {
            nodes.retain(|n| n.size >= min_bytes);
            for node in nodes.iter_mut() {
                retain_large(&mut node.children, min_bytes);
            }
        }
        
        retain_large(&mut nodes, min_bytes);

        let target_node = models::dir_node::DirNode {
            name: root.display().to_string(),
            path: root.clone(),
            size: nodes.iter().map(|n| n.size).sum(),
            children: nodes,
        };

        all_nodes.push(target_node);
    }

    generate_report(&all_nodes, &format, output.as_deref())?;

    Ok(())
}
