use std::path::PathBuf;

pub trait Sentinel {
    
}

impl TreeWatcher {
    pub fn new() -> Self {
        Self {
            watched: Vec::new(),
            running: false,
        }
    }

    pub fn 
}

struct WatchedObject {
    path: PathBuf,
    is_dir: bool,
}
