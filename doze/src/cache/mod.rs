mod local;
pub use local::LocalCache;

use std::error::Error;
use std::fmt;

use crate::ArtifactTag;

#[derive(Debug)]
pub enum CacheError {
    Io(String),
    NotFound(String),
}

impl fmt::Display for CacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CacheError::Io(msg) => write!(f, "cache I/O error: {msg}"),
            CacheError::NotFound(id) => write!(f, "not found in cache: {id}"),
        }
    }
}

impl Error for CacheError {}

// This is the actual super-trait that should be implemented for `execute()`.
pub trait Cache: ArtifactStore + PrimordialLedger {}
impl<T: ArtifactStore + PrimordialLedger> Cache for T {}

pub trait ArtifactStore {
    // Returns true if valid outputs for this Rule are already available,
    // either on disk or fetchable from this store, given the source checksum.
    fn is_fresh(&self, rule_checksum: &str, source_checksum: &str) -> Result<bool, CacheError>;

    // Records the outputs produced by the execution of this Rule.
    fn store_artifacts(
        &mut self,
        rule_checksum: &str,
        source_checksum: &str,
        from: &[ArtifactTag],
    ) -> Result<(), CacheError>;

    // Makes the Rule's outputs available at `into`. Does nothing if already in place.
    // Returns true if at least
    fn ensure_artifacts(
        &self,
        rule_checksum: &str,
        source_checksum: &str,
        into: &[ArtifactTag],
    ) -> Result<(), CacheError>;
}

// The PrimordialLedger stores a list of Rule that were considered primordial rules in the last
// recorded `doze` run. During a run, it stores the Rules in-memory, and at the end it writes them
// to the disk permanently.
pub trait PrimordialLedger {
    // Return true if the Rule is stored in the permanent ledger.
    fn was_rule_in_last_run(&self, rule_checksum: &str, source_checksum: &str) -> bool;

    // Store a Rule in the in-memory ledger.
    fn record_rule(&mut self, rule_checksum: &str, source_checksum: &str);

    // Copy the in-memory ledger into the permanent one.
    // Clear the in-memory ledger.
    fn flush(&mut self) -> Result<(), CacheError>;
}
