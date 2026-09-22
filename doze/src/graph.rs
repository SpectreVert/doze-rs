use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use crate::artifact::{Artifact, ArtifactTag};
use crate::procedure::{ProcedureId, Registry};
use crate::rule::Rule;

#[derive(Debug)]
pub enum AddRuleError {
    DuplicateOutput(ArtifactTag),
    DuplicateRule(String),
    NoInputs,
    NoOutputs,
    ProcedureNotFound(String),
}

impl fmt::Display for AddRuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AddRuleError::DuplicateOutput(tag) => {
                write!(f, "artifact ({}) was registered twice as an output", tag.0)
            }
            AddRuleError::DuplicateRule(checksum) => {
                write!(f, "rule [{}] already exists with the same data", checksum)
            }
            AddRuleError::NoInputs => write!(f, "no inputs provided"),
            AddRuleError::NoOutputs => write!(f, "no outputs provided"),
            AddRuleError::ProcedureNotFound(id) => write!(f, "procedure not found: {}", id),
        }
    }
}

pub struct Graph {
    pub(crate) rules: HashMap<String, Rule>,
    pub(crate) artifacts: HashMap<ArtifactTag, Artifact>,
    // varables: Vec<Variables>,
}

impl Graph {
    pub fn new() -> Self {
        Self {
            rules: HashMap::new(),
            artifacts: HashMap::new(),
        }
    }

    pub fn add_rule(
        &mut self,
        inputs: Vec<String>,
        outputs: Vec<String>,
        input_dir: String,
        output_dir: String,
        proc_id: ProcedureId,
        registry: &Registry,
    ) -> Result<String, AddRuleError> {
        if inputs.is_empty() {
            return Err(AddRuleError::NoInputs);
        }
        if outputs.is_empty() {
            return Err(AddRuleError::NoOutputs);
        }
        if !registry.contains(&proc_id.0) {
            return Err(AddRuleError::ProcedureNotFound(proc_id.0.clone()));
        }

        let input_tags: Vec<ArtifactTag> = inputs
            .into_iter()
            .map(|tag| {
                ArtifactTag(
                    Path::new(&input_dir)
                        .join(tag)
                        .to_string_lossy()
                        .into_owned(),
                )
            })
            .collect();
        let output_tags: Vec<ArtifactTag> = outputs
            .into_iter()
            .map(|tag| {
                ArtifactTag(
                    Path::new(&output_dir)
                        .join(tag)
                        .to_string_lossy()
                        .into_owned(),
                )
            })
            .collect();

        for tag in &output_tags {
            if self
                .artifacts
                .get(tag)
                .and_then(|a| a.creator.as_ref())
                .is_some()
            {
                return Err(AddRuleError::DuplicateOutput(tag.clone()));
            }
        }

        let mut rule = Rule::new(proc_id, input_tags, output_tags);
        let checksum = rule.checksum().to_string();

        if self.rules.contains_key(&checksum) {
            return Err(AddRuleError::DuplicateRule(checksum));
        }

        for tag in &rule.input_tags {
            self.artifacts
                .entry(tag.clone())
                .or_insert_with(|| Artifact::new(tag.clone()))
                .consumers
                .push(checksum.clone());
        }
        for tag in &rule.output_tags {
            self.artifacts
                .entry(tag.clone())
                .or_insert_with(|| Artifact::new(tag.clone()))
                .creator = Some(checksum.clone());
        }

        tracing::debug!(
            rule_id = %checksum,
            proc_id = ?rule.proc_id,
            input_tags = ?rule.input_tags,
            output_tags = ?rule.output_tags,
            "Rule added to Graph");
        self.rules.insert(checksum.clone(), rule);
        Ok(checksum)
    }
}

impl Default for Graph {
    fn default() -> Self {
        Self::new()
    }
}
