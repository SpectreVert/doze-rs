use std::error::Error;
use std::fmt;
use std::fs;

use md5::{Digest, Md5};

use crate::{ArtifactTag, ProcedureError, ProcedureId, Registry};

// pub enum ArtifactSelector {
//     Inputs,
//     Outputs,
//     Both,
//     None,
// }
// pub struct RuleOptions {
//     pub ordered: ArtifactSelector,
//     pub grouped: ArtifactSelector,
// }

#[derive(Debug)]
pub enum RuleError {
    Procedure(ProcedureError),
    SourceReadFailed(String, String),
}

impl fmt::Display for RuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuleError::Procedure(e) => write!(f, "{e}"),
            RuleError::SourceReadFailed(tag, msg) => {
                write!(f, "could not open artifact ({tag}) for reading: {msg}")
            }
        }
    }
}

impl Error for RuleError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RuleError::Procedure(e) => Some(e),
            _ => None,
        }
    }
}

pub struct Rule {
    pub(crate) proc_id: ProcedureId,
    pub(crate) input_tags: Vec<ArtifactTag>,
    pub(crate) output_tags: Vec<ArtifactTag>,
    checksum: String,
}

impl Rule {
    pub fn input_tags(&self) -> &[ArtifactTag] {
        &self.input_tags
    }

    pub fn output_tags(&self) -> &[ArtifactTag] {
        &self.output_tags
    }

    pub(crate) fn new(
        proc_id: ProcedureId,
        mut input_tags: Vec<ArtifactTag>,
        mut output_tags: Vec<ArtifactTag>,
    ) -> Self {
        input_tags.sort();
        output_tags.sort();
        Self {
            proc_id,
            input_tags,
            output_tags,
            checksum: String::new(),
        }
    }

    pub(crate) fn checksum(&mut self) -> &str {
        if !self.checksum.is_empty() {
            return &self.checksum;
        }
        let mut hasher = Md5::new();
        for tag in &self.input_tags {
            hasher.update(tag.0.as_bytes());
        }
        for tag in &self.output_tags {
            hasher.update(tag.0.as_bytes());
        }
        hasher.update(self.proc_id.0.as_bytes());

        self.checksum = hex::encode(hasher.finalize());
        &self.checksum
    }

    pub(crate) fn source_checksum(&self) -> Result<String, RuleError> {
        let mut hasher = Md5::new();
        for tag in &self.input_tags {
            let content = fs::read(&tag.0)
                .map_err(|e| RuleError::SourceReadFailed(tag.0.clone(), e.to_string()))?;
            hasher.update(&content);
        }
        Ok(hex::encode(hasher.finalize()))
    }

    pub(crate) fn execute(&mut self, registry: &Registry) -> Result<(), RuleError> {
        let mut proc = registry
            .create(&self.proc_id.0)
            .map_err(RuleError::Procedure)?;

        proc.exec(self).map_err(RuleError::Procedure)?;
        tracing::debug!(
            proc_id = %self.proc_id.0,
            input_tags = ?self.input_tags,
            output_tags = ?self.output_tags,
            "executed Procedure on Rule",
        );

        Ok(())
    }
}
