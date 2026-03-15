use serde::Serialize;
use std::path::PathBuf;

/// A node in the scanned directory tree.
#[derive(Serialize, Clone)]
pub struct DirNode {
    pub name: String,
    pub path: PathBuf,
    pub size: u64,
    pub children: Vec<DirNode>,
}
