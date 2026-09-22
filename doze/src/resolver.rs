use std::collections::VecDeque;
use std::error::Error;
use std::fmt;

use crate::Graph;
use crate::PrimordialLedger;
use crate::RuleError;

pub struct Plan {
    // The IDs of the Rules that are planned.
    pub rules: Vec<String>,
}

impl Plan {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }
}

impl Default for Plan {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub enum PlanningError {
    Cycle,
    SourceUnreadable(RuleError),
}

impl fmt::Display for PlanningError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlanningError::Cycle => write!(f, "cycle detected in Graph"),
            PlanningError::SourceUnreadable(e) => {
                write!(f, "could not compute source checksum: {e}")
            }
        }
    }
}

impl Error for PlanningError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PlanningError::SourceUnreadable(e) => Some(e),
            PlanningError::Cycle => None,
        }
    }
}

pub enum ResolveMode {
    Terse,
    Full,
}

pub trait Resolver {
    fn resolve(
        &self,
        mode: ResolveMode,
        graph: &Graph,
        ledger: &mut dyn PrimordialLedger,
    ) -> Result<Plan, PlanningError>;
}

// The TopologicalResolver does Kahn's algorithm to order the plan linearly.
pub struct TopologicalResolver {}

impl Resolver for TopologicalResolver {
    fn resolve(
        &self,
        mode: ResolveMode,
        graph: &Graph,
        ledger: &mut dyn PrimordialLedger,
    ) -> Result<Plan, PlanningError> {
        let _span = tracing::info_span!("resolve", rule_count = graph.rules.len()).entered();

        let mut plan = Plan::new();
        let mut queue: VecDeque<String> = VecDeque::new();

        // A batch is a group of artifacts. When such group is part of a Rule,
        // it can be either an input or output batch.
        // For now, we are looking for input batches that have no parents Rules.
        'rule_loop: for (rule_id, rule) in graph.rules.iter() {
            for input_tag in rule.input_tags.iter() {
                if graph
                    .artifacts
                    .get(input_tag)
                    .unwrap()
                    .creator_rule()
                    .is_some()
                {
                    continue 'rule_loop;
                }
            }

            // Rule must be registered in the PrimordialLedger.
            let source_checksum = rule
                .source_checksum()
                .map_err(PlanningError::SourceUnreadable)?;

            if !ledger.was_rule_in_last_run(rule_id, &source_checksum)
                || matches!(mode, ResolveMode::Full)
            {
                queue.push_back(rule_id.clone());
            }
        }

        // Loop until the queue becomes empty.
        while !queue.is_empty() {
            let rule_id = queue.pop_front().unwrap();
            plan.rules.push(rule_id.clone());

            // For each output Artifact of this Rule, check their consumer Rules to see
            // if they are ready to be scheduled.
            for output_id in graph.rules.get(&rule_id).unwrap().output_tags.iter() {
                'consumer_rule_loop: for consumer_rule_id in graph
                    .artifacts
                    .get(output_id)
                    .unwrap()
                    .consumer_rules()
                    .iter()
                {
                    for consumer_input_id in
                        graph.rules.get(consumer_rule_id).unwrap().input_tags.iter()
                    {
                        if output_id == consumer_input_id {
                            continue;
                        } else {
                            match graph
                                .artifacts
                                .get(consumer_input_id)
                                .unwrap()
                                .creator_rule()
                            {
                                None => continue,
                                Some(creator_rule_id) => {
                                    match plan.rules.iter().position(|scheduled_rule_id| {
                                        *scheduled_rule_id == creator_rule_id
                                    }) {
                                        None => continue 'consumer_rule_loop,
                                        Some(_) => continue,
                                    }
                                }
                            }
                        }
                    }
                    // Maybe fixme...
                    // We might need to check first if we didn't already schedule the rule.
                    queue.push_back(consumer_rule_id.clone());
                }
            }
        }

        if plan.rules.len() != graph.rules.len() {
            tracing::error!(
                planned = plan.rules.len(),
                total = graph.rules.len(),
                "{}",
                PlanningError::Cycle
            );
            return Err(PlanningError::Cycle);
        }

        tracing::info!(planned = plan.rules.len());
        Ok(plan)
    }
}

#[cfg(test)]
mod tests {
    /* Tests different type of Graphs. */
    use std::path::Path;
    use tempfile::TempDir;

    use super::*;
    use crate::procedure::{Procedure, ProcedureError, ProcedureId, Registry};
    use crate::CacheError;
    use crate::Graph;

    struct NullLedger;
    impl PrimordialLedger for NullLedger {
        fn was_rule_in_last_run(&self, _rule_checksum: &str, _source_checksum: &str) -> bool {
            false
        }

        fn record_rule(&mut self, _rule_checksum: &str, _source_checksum: &str) {}
        fn flush(&mut self) -> Result<(), CacheError> {
            Ok(())
        }
    }

    struct NoOp;
    impl Procedure for NoOp {
        fn exec(&mut self, _rule: &mut crate::rule::Rule) -> Result<(), ProcedureError> {
            Ok(())
        }
    }

    fn registry() -> Registry {
        let mut reg = Registry::new();
        reg.register("noop".to_string(), || Box::new(NoOp));
        reg
    }

    fn add(
        graph: &mut Graph,
        input_tags: &[&str],
        output_tags: &[&str],
        registry: &Registry,
        dir: &Path,
    ) -> String {
        for input_tag in input_tags {
            let path = dir.join(input_tag);
            if !path.exists() {
                std::fs::write(dir.join(input_tag), b"placeholder").unwrap();
            }
        }

        let dir_str = dir.to_string_lossy().into_owned();
        graph
            .add_rule(
                input_tags.iter().map(|s| s.to_string()).collect(),
                output_tags.iter().map(|s| s.to_string()).collect(),
                dir_str.clone(),
                dir_str,
                ProcedureId("noop".to_string()),
                registry,
            )
            .expect("add_rule should succeed")
    }

    fn pos(plan: &Plan, checksum: &str) -> usize {
        plan.rules
            .iter()
            .position(|id| id == checksum)
            .unwrap_or_else(|| panic!("rule [{checksum}] missing from plan"))
    }

    #[test]
    fn linear() {
        let mut graph = Graph::new();
        let registry = registry();
        let dir = TempDir::new().unwrap();

        let a = add(&mut graph, &["first"], &["second"], &registry, dir.path());
        let b = add(&mut graph, &["second"], &["third"], &registry, dir.path());
        let c = add(&mut graph, &["third"], &["final"], &registry, dir.path());

        let plan = TopologicalResolver {}
            .resolve(ResolveMode::Full, &graph, &mut NullLedger)
            .unwrap();

        assert_eq!(plan.rules.len(), 3);
        assert!(pos(&plan, &a) < pos(&plan, &b));
        assert!(pos(&plan, &b) < pos(&plan, &c));
    }

    #[test]
    fn diamond() {
        let mut graph = Graph::new();
        let registry = registry();
        let dir = TempDir::new().unwrap();

        let a = add(&mut graph, &["first"], &["second"], &registry, dir.path());
        let b = add(&mut graph, &["second"], &["third"], &registry, dir.path());
        let c = add(&mut graph, &["second"], &["fourth"], &registry, dir.path());
        let d = add(
            &mut graph,
            &["third", "fourth"],
            &["final"],
            &registry,
            dir.path(),
        );

        let plan = TopologicalResolver {}
            .resolve(ResolveMode::Full, &graph, &mut NullLedger)
            .unwrap();

        assert_eq!(plan.rules.len(), 4);
        assert!(pos(&plan, &a) < pos(&plan, &b));
        assert!(pos(&plan, &a) < pos(&plan, &c));
        assert!(pos(&plan, &b) < pos(&plan, &d));
        assert!(pos(&plan, &c) < pos(&plan, &d));
    }

    #[test]
    fn disconnected() {
        let mut graph = Graph::new();
        let registry = registry();
        let dir = TempDir::new().unwrap();

        let a1 = add(&mut graph, &["src1"], &["out1"], &registry, dir.path());
        let a2 = add(&mut graph, &["out1"], &["out2"], &registry, dir.path());
        let b1 = add(&mut graph, &["src2"], &["out3"], &registry, dir.path());

        let plan = TopologicalResolver {}
            .resolve(ResolveMode::Full, &graph, &mut NullLedger)
            .unwrap();

        // No ordering assumed between the two chains.
        assert_eq!(plan.rules.len(), 3);
        assert!(pos(&plan, &a1) < pos(&plan, &a2));
        let _ = b1;
    }

    #[test]
    fn multiple_primordial_inputs() {
        let mut graph = Graph::new();
        let registry = registry();
        let dir = TempDir::new().unwrap();

        let r = add(
            &mut graph,
            &["first", "second"],
            &["final"],
            &registry,
            dir.path(),
        );

        let plan = TopologicalResolver {}
            .resolve(ResolveMode::Full, &graph, &mut NullLedger)
            .unwrap();

        assert_eq!(plan.rules.len(), 1);
        assert_eq!(plan.rules[0], r);
    }

    #[test]
    fn all_rules_present_exactly_once() {
        let mut graph = Graph::new();
        let registry = registry();
        let dir = TempDir::new().unwrap();

        add(&mut graph, &["first"], &["second"], &registry, dir.path());
        add(&mut graph, &["second"], &["third"], &registry, dir.path());
        add(&mut graph, &["second"], &["fourth"], &registry, dir.path());
        add(
            &mut graph,
            &["third", "fourth"],
            &["final"],
            &registry,
            dir.path(),
        );

        let plan = TopologicalResolver {}
            .resolve(ResolveMode::Full, &graph, &mut NullLedger)
            .unwrap();

        let mut seen = std::collections::HashSet::new();
        for id in &plan.rules {
            assert!(seen.insert(id.clone()), "rule {id} scheduled twice");
        }
        assert_eq!(plan.rules.len(), graph.rules.len());
    }

    #[test]
    fn cyclic() {
        let mut graph = Graph::new();
        let registry = registry();
        let dir = TempDir::new().unwrap();

        add(&mut graph, &["first"], &["second"], &registry, dir.path());
        add(&mut graph, &["second"], &["third"], &registry, dir.path());
        add(&mut graph, &["third"], &["first"], &registry, dir.path());

        let result = TopologicalResolver {}.resolve(ResolveMode::Full, &graph, &mut NullLedger);

        assert!(matches!(result, Err(PlanningError::Cycle)));
    }

    mod ordering {
        /* Tests out-of-order Graphs. */

        use std::collections::HashMap;

        use rand::rngs::StdRng;
        use rand::seq::SliceRandom;
        use rand::{RngExt, SeedableRng};

        use super::*;
        use crate::graph::Graph;

        #[test]
        fn diamond_reversed() {
            let mut graph = Graph::new();
            let registry = registry();
            let dir = TempDir::new().unwrap();

            let d = add(
                &mut graph,
                &["third", "fourth"],
                &["final"],
                &registry,
                dir.path(),
            );
            let c = add(&mut graph, &["second"], &["fourth"], &registry, dir.path());
            let b = add(&mut graph, &["second"], &["third"], &registry, dir.path());
            let a = add(&mut graph, &["first"], &["second"], &registry, dir.path());

            let plan = TopologicalResolver {}
                .resolve(ResolveMode::Full, &graph, &mut NullLedger)
                .unwrap();

            assert_eq!(plan.rules.len(), 4);
            assert!(pos(&plan, &a) < pos(&plan, &b));
            assert!(pos(&plan, &a) < pos(&plan, &c));
            assert!(pos(&plan, &b) < pos(&plan, &d));
            assert!(pos(&plan, &c) < pos(&plan, &d));
        }

        #[test]
        fn diamond_random() {
            let seed: u64 = rand::rng().random();
            let mut rng = StdRng::seed_from_u64(seed);

            let mut graph = Graph::new();
            let registry = registry();
            let dir = TempDir::new().unwrap();

            let mut specs: Vec<(&[&str], &[&str])> = vec![
                (&["first"], &["second"]),
                (&["second"], &["third"]),
                (&["second"], &["fourth"]),
                (&["third", "fourth"], &["final"]),
            ];
            specs.shuffle(&mut rng);

            let mut checksums = HashMap::new();
            for (inputs, outputs) in &specs {
                let id = add(&mut graph, inputs, outputs, &registry, dir.path());
                checksums.insert(outputs[0], id);
            }

            let plan = TopologicalResolver {}
                .resolve(ResolveMode::Full, &graph, &mut NullLedger)
                .unwrap_or_else(|e| panic!("seed {seed} failed: {e}"));

            assert!(pos(&plan, &checksums["second"]) < pos(&plan, &checksums["third"]));
            assert!(pos(&plan, &checksums["second"]) < pos(&plan, &checksums["fourth"]));
            assert!(pos(&plan, &checksums["third"]) < pos(&plan, &checksums["final"]));
            assert!(pos(&plan, &checksums["fourth"]) < pos(&plan, &checksums["final"]));
        }
    }
}
