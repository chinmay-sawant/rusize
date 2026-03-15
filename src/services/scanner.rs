use crate::models::dir_node::DirNode;
use std::fs;
use std::path::Path;

/// Recursively scan `path`, collecting child nodes up to `max_depth` levels.
pub fn scan_tree(path: &Path, current_depth: usize, max_depth: usize) -> DirNode {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string());

    let mut node = DirNode {
        name,
        path: path.to_path_buf(),
        size: 0,
        children: Vec::new(),
    };

    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(_) => return node,
    };

    for entry in entries.flatten() {
        let md = match entry.metadata() {
            Ok(md) => md,
            Err(_) => continue,
        };

        if md.is_dir() {
            if current_depth < max_depth {
                let child = scan_tree(&entry.path(), current_depth + 1, max_depth);
                node.size += child.size;
                node.children.push(child);
            } else {
                node.size += dir_size_flat(&entry.path());
            }
        } else {
            node.size += md.len();
        }
    }

    node
}

fn dir_size_flat(path: &Path) -> u64 {
    let mut total: u64 = 0;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(md) = entry.metadata() {
                if md.is_dir() {
                    total += dir_size_flat(&entry.path());
                } else {
                    total += md.len();
                }
            }
        }
    }
    total
}

pub fn sort_recursive(nodes: &mut [DirNode]) {
    nodes.sort_by(|a, b| b.size.cmp(&a.size));
    for node in nodes.iter_mut() {
        sort_recursive(&mut node.children);
    }
}
