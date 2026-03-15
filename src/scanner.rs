use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::thread;

/// A node in the scanned directory tree.
///
/// `size` always represents the **total recursive size** of the directory
/// (including all descendants), regardless of whether children were tracked.
/// `children` are only populated up to the requested `max_depth`.
pub struct DirNode {
    pub name: String,
    pub path: PathBuf,
    pub size: u64,
    pub has_children: bool,
    pub children_loaded: bool,
    pub is_loading: bool,
    pub children: Vec<DirNode>,
}

/// Recursively scan `path`, collecting child nodes up to `max_depth` levels.
///
/// The `size` field is always fully recursive — it accounts for every file
/// in every subdirectory, even beyond the display depth. Only `children`
/// are limited by `max_depth` (to keep memory usage reasonable).
///
/// # Arguments
/// * `path`          – Directory to scan
/// * `current_depth` – Current recursion depth (start at 1 for top-level)
/// * `max_depth`     – Maximum depth at which to track child nodes
pub fn scan_tree(path: &Path, current_depth: usize, max_depth: usize) -> DirNode {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string());

    let mut node = DirNode {
        name,
        path: path.to_path_buf(),
        size: 0,
        has_children: false,
        children_loaded: false,
        is_loading: false,
        children: Vec::new(),
    };

    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(_) => return node, // Permission denied or inaccessible
    };

    for entry in entries.flatten() {
        let md = match entry.metadata() {
            Ok(md) => md,
            Err(_) => continue,
        };

        if md.is_dir() {
            node.has_children = true;
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

    node.children_loaded = !node.has_children || current_depth < max_depth;

    node
}

pub struct ScanResult {
    pub target_idx: usize,
    pub node_path: Vec<usize>,
    pub children: Vec<DirNode>,
}

pub fn spawn_load_children(
    target_idx: usize,
    node_path: Vec<usize>,
    dir_path: PathBuf,
    tx: Sender<ScanResult>,
) {
    thread::spawn(move || {
        let mut children = scan_direct_children(&dir_path);
        sort_recursive(&mut children);
        let _ = tx.send(ScanResult {
            target_idx,
            node_path,
            children,
        });
    });
}

fn scan_direct_children(path: &Path) -> Vec<DirNode> {
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    let mut children = Vec::new();
    for entry in entries.flatten() {
        let md = match entry.metadata() {
            Ok(md) => md,
            Err(_) => continue,
        };

        if md.is_dir() {
            children.push(scan_tree(&entry.path(), 1, 1));
        }
    }

    children
}

/// Calculate the total size of a directory recursively (no children tracked).
///
/// This is the fast path used once we've exceeded `max_depth` — it only
/// accumulates byte counts without allocating `DirNode` objects.
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

/// Sort a slice of `DirNode` by size (largest first), recursively.
pub fn sort_recursive(nodes: &mut [DirNode]) {
    nodes.sort_by(|a, b| b.size.cmp(&a.size));
    for node in nodes.iter_mut() {
        sort_recursive(&mut node.children);
    }
}
