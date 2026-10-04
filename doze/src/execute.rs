use std::error::Error;
use std::fmt;
use std::time;

use tracing::{debug, info, info_span};

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
    let start_time = time::Instant::now();
    let _span = info_span!("execute").entered();
    info!(rules_nb = graph.rules.len(), "starting");

    let mut executed = 0;
    let mut fetched = 0;

    for rule_checksum in &plan.rules {
        let _rule_span = info_span!("rule", id = rule_checksum).entered();

        let rule = graph
            .rules
            .get_mut(rule_checksum)
            .ok_or_else(|| ExecuteError::RuleNotFound(rule_checksum.clone()))?;
        let source_checksum = rule.source_checksum().map_err(ExecuteError::Rule)?;

        if cache
            .is_fresh(rule_checksum, &source_checksum)
            .map_err(ExecuteError::Cache)?
        {
            cache
                .ensure_artifacts(rule_checksum, &source_checksum, &rule.output_tags)
                .map_err(ExecuteError::Cache)?;
            info!("cached");
            fetched += 1;
        } else {
            rule.execute(registry).map_err(ExecuteError::Rule)?;
            info!("created");
            executed += 1;

            cache
                .store_artifacts(rule_checksum, &source_checksum, &rule.output_tags)
                .map_err(ExecuteError::Cache)?;
        }
    }

    cache.flush().map_err(ExecuteError::Cache)?;
    debug!(executed, fetched, elapsed = ?start_time.elapsed(), "complete");

    Ok(ExecutionReport { executed, fetched })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use tempfile::TempDir;

    use super::*;
    use crate::procedure::{Procedure, ProcedureError, ProcedureId};
    use crate::resolver::{ResolveMode, Resolver, TopologicalResolver};
    use crate::rule::Rule;
    use crate::LocalCache;

    // Copies inputs to outputs, writing in place like most real tools do.
    struct Copy;
    impl Procedure for Copy {
        fn exec(&mut self, rule: &mut Rule) -> Result<(), ProcedureError> {
            for (i, o) in rule.input_tags().iter().zip(rule.output_tags()) {
                fs::copy(&i.0, &o.0).map_err(|e| ProcedureError::ExecFailed(e.to_string()))?;
            }
            Ok(())
        }
    }

    fn run(dir: &Path) -> ExecutionReport {
        let mut registry = Registry::new();
        registry.register("copy".to_string(), || Box::new(Copy));

        let d = dir.to_string_lossy().into_owned();
        let mut graph = Graph::new();
        for (i, o) in [("root", "mid"), ("mid", "leaf")] {
            graph
                .add_rule(
                    vec![i.into()],
                    vec![o.into()],
                    d.clone(),
                    d.clone(),
                    ProcedureId("copy".to_string()),
                    &registry,
                )
                .unwrap();
        }

        let mut cache = LocalCache::new(dir.join(".doze")).unwrap();
        let plan = TopologicalResolver {}
            .resolve(ResolveMode::Full, &graph, &mut cache)
            .unwrap();
        execute(&plan, &mut graph, &registry, &mut cache).unwrap()
    }

    #[test]
    fn changing_inputs_does_not_corrupt_cache() {
        let dir = TempDir::new().unwrap();
        let leaf = dir.path().join("leaf");
        let root = dir.path().join("root");

        fs::write(&root, "v1").unwrap();
        assert_eq!(run(dir.path()).executed, 2);
        assert_eq!(run(dir.path()).fetched, 2);

        fs::write(&root, "v2").unwrap();
        assert_eq!(run(dir.path()).executed, 2);

        // Reverting fetches the v1 outputs, which are now hardlinks into the cache.
        fs::write(&root, "v1").unwrap();
        assert_eq!(run(dir.path()).fetched, 2);
        assert_eq!(fs::read_to_string(&leaf).unwrap(), "v1");

        // Rebuilding on top of those hardlinks must not write v3 into the v1 cache entries.
        fs::write(&root, "v3").unwrap();
        assert_eq!(run(dir.path()).executed, 2);
        assert_eq!(fs::read_to_string(&leaf).unwrap(), "v3");

        fs::write(&root, "v1").unwrap();
        assert_eq!(run(dir.path()).fetched, 2);
        assert_eq!(fs::read_to_string(&leaf).unwrap(), "v1");
    }
}
