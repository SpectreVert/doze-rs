use std::collections::{HashMap, VecDeque};
use std::error::Error;
use std::fmt;
use std::time;

use tracing::{debug, error, info_span};

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
            PlanningError::Cycle => write!(f, "cycle in graph"),
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
        let start_time = time::Instant::now();
        let _span = info_span!("resolve").entered();
        debug!(graph_rules_nb = graph.rules.len(), "starting");

        // The plan contains the ordered list of Rules that must be executed to bring the Graph up-to-date.
        let mut plan = Plan::new();
        // The queue holds the Rules whose dependencies (if any) have already gone through it,
        // and a marker for dirtyness which indicates if the Rule must be put in the plan.
        let mut queue: VecDeque<(&str, bool)> = VecDeque::with_capacity(graph.rules.len());
        // Pending tracks, for each Rule with dependencies, the amount of dependencies that haven't
        // gone through the queue yet, and whether any of these dependencies need to run.
        let mut pending: HashMap<&str, (usize, bool)> = HashMap::with_capacity(graph.rules.len());
        // Count to keep track of the amount of Rules that were ordered and considered for planning.
        // If ordered_nb != graph.rules.len(), then we have a cycle in the Graph.
        let mut ordered_nb = 0;

        // First we cycle through the Graph to find primordial Rules, i.e Rules which have no dependencies.
        for (rule_id, rule) in graph.rules.iter() {
            let parents_nb = rule
                .input_tags
                .iter()
                .filter(|tag| graph.artifacts.get(*tag).unwrap().creator_rule().is_some())
                .count();

            if parents_nb > 0 {
                // Add to pending any non-primordial Rules. We don't know yet if they are dirty.
                pending.insert(rule_id, (parents_nb, false));
                continue;
            }

            // This is a primordial Rule, add it to the queue (it is by definition ready for ordering).
            // Register it in the ledger so that next run we know immediately if it is dirty or not.
            let source_checksum = rule
                .source_checksum()
                .map_err(PlanningError::SourceUnreadable)?;

            let is_dirty = !ledger.was_rule_in_last_run(rule_id, &source_checksum)
                || matches!(mode, ResolveMode::Full);

            ledger.record_rule(rule_id, &source_checksum);
            queue.push_back((rule_id, is_dirty));
        }

        // Every Rule is ordered and makes it into the queue. This happens regardless of the mode
        // and helps us always detect cycles. However, only dirty Rules make it into the Plan.
        while let Some((rule_id, is_dirty)) = queue.pop_front() {
            ordered_nb += 1;
            if is_dirty {
                plan.rules.push(rule_id.to_string());
            }

            for output_tag in graph.rules.get(rule_id).unwrap().output_tags.iter() {
                for consumer_rule_id in graph.artifacts.get(output_tag).unwrap().consumer_rules() {
                    let consumer_rule_id = consumer_rule_id.as_str();
                    let (parents_nb, consumer_dirty) = pending.get_mut(consumer_rule_id).unwrap();

                    *parents_nb -= 1;
                    *consumer_dirty |= is_dirty;

                    // Ready for ordering.
                    if *parents_nb == 0 {
                        queue.push_back((consumer_rule_id, *consumer_dirty));
                    }
                }
            }
        }

        if ordered_nb != graph.rules.len() {
            error!(
                ordered_nb,
                total_nb = graph.rules.len(),
                "{}",
                PlanningError::Cycle
            );
            return Err(PlanningError::Cycle);
        }

        debug!(planned_nb = plan.rules.len(), elapsed = ?start_time.elapsed(), "complete");
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

    #[test]
    fn cyclic_terse() {
        let mut graph = Graph::new();
        let registry = registry();
        let dir = TempDir::new().unwrap();

        add(&mut graph, &["src"], &["first"], &registry, dir.path());
        add(
            &mut graph,
            &["first", "third"],
            &["second"],
            &registry,
            dir.path(),
        );
        add(&mut graph, &["second"], &["third"], &registry, dir.path());

        let result = TopologicalResolver {}.resolve(ResolveMode::Terse, &graph, &mut NullLedger);

        assert!(matches!(result, Err(PlanningError::Cycle)));
    }

    // Reports every Rule in `unchanged` as part of the last run.
    struct LastRunLedger {
        unchanged: Vec<String>,
    }
    impl PrimordialLedger for LastRunLedger {
        fn was_rule_in_last_run(&self, rule_checksum: &str, _source_checksum: &str) -> bool {
            self.unchanged.iter().any(|id| id == rule_checksum)
        }

        fn record_rule(&mut self, _rule_checksum: &str, _source_checksum: &str) {}
        fn flush(&mut self) -> Result<(), CacheError> {
            Ok(())
        }
    }

    #[test]
    fn terse_one_parent_unchanged() {
        let mut graph = Graph::new();
        let registry = registry();
        let dir = TempDir::new().unwrap();

        let a = add(&mut graph, &["src1"], &["first"], &registry, dir.path());
        let b = add(&mut graph, &["src2"], &["second"], &registry, dir.path());
        let c = add(
            &mut graph,
            &["first", "second"],
            &["final"],
            &registry,
            dir.path(),
        );

        let mut ledger = LastRunLedger { unchanged: vec![b] };
        let plan = TopologicalResolver {}
            .resolve(ResolveMode::Terse, &graph, &mut ledger)
            .unwrap();

        assert_eq!(plan.rules, vec![a, c]);
    }

    #[test]
    fn consumer_of_several_outputs_of_one_rule() {
        let mut graph = Graph::new();
        let registry = registry();
        let dir = TempDir::new().unwrap();

        let a = add(
            &mut graph,
            &["src"],
            &["first", "second"],
            &registry,
            dir.path(),
        );
        let b = add(
            &mut graph,
            &["first", "second"],
            &["final"],
            &registry,
            dir.path(),
        );

        let plan = TopologicalResolver {}
            .resolve(ResolveMode::Full, &graph, &mut NullLedger)
            .unwrap();

        assert_eq!(plan.rules, vec![a, b]);
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
