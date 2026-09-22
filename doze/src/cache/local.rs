use std::fs;
use std::path::{Path, PathBuf};

use super::{ArtifactStore, CacheError, PrimordialLedger};
use crate::artifact::ArtifactTag;

// Local (on-disk only) cache that implements ArtifactStore and PrimordialLedger.
pub struct LocalCache {
    // Shared fields.
    root: PathBuf,

    // PrimordialLedger fields.
    recorded: Vec<String>,
}

impl LocalCache {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self, CacheError> {
        let root = root.into();
        fs::create_dir_all(&root).map_err(|e| CacheError::Io(e.to_string()))?;
        Ok(Self {
            root,
            recorded: Vec::new(),
        })
    }

    fn outputs_dir(&self, rule_checksum: &str, source_checksum: &str) -> PathBuf {
        self.root.join(rule_checksum).join(source_checksum)
    }

    fn last_run_dir(&self) -> PathBuf {
        self.root.join("last_run")
    }

    fn key(rule_checksum: &str, source_checksum: &str) -> String {
        format!("{rule_checksum}_{source_checksum}")
    }
}

impl ArtifactStore for LocalCache {
    fn is_fresh(&self, rule_checksum: &str, source_checksum: &str) -> Result<bool, CacheError> {
        Ok(self.outputs_dir(rule_checksum, source_checksum).exists())
    }

    fn store_artifacts(
        &mut self,
        rule_checksum: &str,
        source_checksum: &str,
        from: &[ArtifactTag],
    ) -> Result<(), CacheError> {
        let outputs_dir = self.outputs_dir(rule_checksum, source_checksum);
        if outputs_dir.exists() {
            return Ok(());
        }

        let tmp_dir = tmp_marker_path(&outputs_dir);
        fs::create_dir_all(&tmp_dir).map_err(|e| CacheError::Io(e.to_string()))?;

        for tag in from {
            let link_path = tmp_dir.join(tag.0.trim_start_matches("/"));
            if let Some(parent) = link_path.parent() {
                fs::create_dir_all(parent).map_err(|e| CacheError::Io(e.to_string()))?;
            }
            fs::copy(&tag.0, &link_path).map_err(|e| CacheError::Io(e.to_string()))?;
        }

        fs::rename(&tmp_dir, &outputs_dir).map_err(|e| CacheError::Io(e.to_string()))?;
        Ok(())
    }

    fn ensure_artifacts(
        &self,
        rule_checksum: &str,
        source_checksum: &str,
        into: &[ArtifactTag],
    ) -> Result<(), CacheError> {
        let _span = tracing::debug_span!("ensure");
        let outputs_dir = self.outputs_dir(rule_checksum, source_checksum);
        if !outputs_dir.exists() {
            return Err(CacheError::NotFound(format!(
                "{rule_checksum}/{source_checksum}"
            )));
        }

        for tag in into {
            let cached_path = outputs_dir.join(tag.0.trim_start_matches("/")); // @fixme
            let dest = Path::new(&tag.0);

            if dest.exists() {
                if same_file(dest, &cached_path).unwrap_or(false) {
                    tracing::debug!("fetch=skip");
                    continue;
                }
                fs::remove_file(dest).map_err(|e| CacheError::Io(e.to_string()))?;
                tracing::debug!("remove stale output");
            }
            fs::hard_link(&cached_path, dest).map_err(|e| CacheError::Io(e.to_string()))?;
            tracing::debug!("fetch=cache");
        }

        Ok(())
    }
}

// Utils functions for LocalCache's impl of ArtifactStore.
fn tmp_marker_path(outputs_dir: &Path) -> PathBuf {
    let mut s = outputs_dir.as_os_str().to_owned();
    s.push("=");
    PathBuf::from(s)
}

#[cfg(unix)]
fn same_file(a: &Path, b: &Path) -> std::io::Result<bool> {
    use std::os::unix::fs::MetadataExt;
    let (a_meta, b_meta) = (fs::metadata(a)?, fs::metadata(b)?);
    Ok(a_meta.dev() == b_meta.dev() && a_meta.ino() == b_meta.ino())
}

impl PrimordialLedger for LocalCache {
    fn was_rule_in_last_run(&self, rule_checksum: &str, source_checksum: &str) -> bool {
        self.last_run_dir()
            .join(Self::key(rule_checksum, source_checksum))
            .exists()
    }

    fn record_rule(&mut self, rule_checksum: &str, source_checksum: &str) {
        self.recorded
            .push(Self::key(rule_checksum, source_checksum));
    }

    fn flush(&mut self) -> Result<(), CacheError> {
        let last_run_dir = self.last_run_dir();
        fs::create_dir_all(&last_run_dir).map_err(|e| CacheError::Io(e.to_string()))?;

        for entry in fs::read_dir(&last_run_dir).map_err(|e| CacheError::Io(e.to_string()))? {
            let entry = entry.map_err(|e| CacheError::Io(e.to_string()))?;
            fs::remove_dir_all(entry.path()).map_err(|e| CacheError::Io(e.to_string()))?;
        }
        for key in &self.recorded {
            fs::create_dir(last_run_dir.join(key)).map_err(|e| CacheError::Io(e.to_string()))?;
        }
        self.recorded.clear();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn store_then_fetch_roundtrip() {
        let dir = TempDir::new().unwrap();
        let mut cache = LocalCache::new(dir.path()).unwrap();

        let src = dir.path().join("out.txt");
        fs::write(&src, b"hello").unwrap();
        let tag = ArtifactTag(src.to_string_lossy().into_owned());

        cache
            .store_artifacts("rule1", "srcsum1", std::slice::from_ref(&tag))
            .unwrap();
        cache.flush().unwrap();

        assert!(cache.is_fresh("rule1", "srcsum1").unwrap());
        assert!(!cache.is_fresh("rule1", "differrent").unwrap());

        fs::remove_file(&src).unwrap();
        cache.ensure_artifacts("rule1", "srcsum1", &[tag]).unwrap();
        assert!(src.exists());
    }
}
