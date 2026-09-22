use std::error::Error;
use std::fmt;

use crate::cache::{Cache, CacheError};
use crate::graph::Graph;
use crate::procedure::Registry;
use crate::resolver::Plan;
use crate::rule::RuleError;

#[derive(Debug)]
pub enum ExecuteError {
    Cache(CacheError),
    Rule(RuleError),
    RuleNotFound(String),
}

impl fmt::Display for ExecuteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecuteError::Cache(e) => write!(f, "{e}"),
            ExecuteError::Rule(e) => write!(f, "{e}"),
            ExecuteError::RuleNotFound(id) => {
                write!(f, "rule [{id}] resolved for execution but doesn't exist")
            }
        }
    }
}

impl Error for ExecuteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ExecuteError::Cache(e) => Some(e),
            ExecuteError::Rule(e) => Some(e),
            ExecuteError::RuleNotFound(_) => None,
        }
    }
}

#[derive(Debug)]
pub struct ExecutionReport {
    pub executed: usize,
    pub fetched: usize,
}

// A bare-bones single-threaded linear executor.
pub fn execute(
    plan: &Plan,
    graph: &mut Graph,
    registry: &Registry,
    cache: &mut dyn Cache,
) -> Result<ExecutionReport, ExecuteError> {
    let _span = tracing::info_span!("execute", plan_size = plan.rules.len()).entered();

    let mut executed = 0;
    let mut fetched = 0;

    tracing::info!("starting Graph execution");
    for rule_checksum in &plan.rules {
        let _rule_span = tracing::info_span!("rule", rule_id = %rule_checksum).entered();

        let rule = graph
            .rules
            .get_mut(rule_checksum)
            .ok_or_else(|| ExecuteError::RuleNotFound(rule_checksum.clone()))?;
        let source_checksum = rule.source_checksum().map_err(ExecuteError::Rule)?;

        // @FIXME something is very fishy around here...
        if cache
            .is_fresh(rule_checksum, &source_checksum)
            .map_err(ExecuteError::Cache)?
        {
            cache
                .ensure_artifacts(rule_checksum, &source_checksum, &rule.output_tags)
                .map_err(ExecuteError::Cache)?;
            tracing::info!("fetched");
            fetched += 1;
        } else {
            rule.execute(registry).map_err(ExecuteError::Rule)?;
            tracing::info!("produced");
            executed += 1;

            cache
                .store_artifacts(rule_checksum, &source_checksum, &rule.output_tags)
                .map_err(ExecuteError::Cache)?;
        }
    }

    cache.flush().map_err(ExecuteError::Cache)?;
    tracing::info!(executed, fetched, "Graph execution complete");

    Ok(ExecutionReport { executed, fetched })
}
