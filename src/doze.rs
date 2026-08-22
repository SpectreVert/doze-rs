use std::collections::{HashMap, VecDeque};
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProcedureId(pub String);

#[derive(Debug)]
pub enum ProcedureError {
    NotFound(String),
    ExecFailed(String),
}

impl Error for ProcedureError {}

impl fmt::Display for ProcedureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProcedureError::NotFound(id) => {
                write!(f, "procedure not found: {}", id)
            }
            ProcedureError::ExecFailed(msg) => {
                write!(f, "execution failed: {}", msg)
            }
        }
    }
}

pub trait Procedure {
    fn exec(&mut self, rule: &mut Rule) -> Result<(), ProcedureError>;
}

pub type ProcedureFactory = Box<dyn Fn() -> Box<dyn Procedure>>;

pub struct Registry {
    procedures: HashMap<String, ProcedureFactory>,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            procedures: HashMap::new(),
        }
    }

    pub fn register<F>(&mut self, id: String, factory: F)
    where
        F: Fn() -> Box<dyn Procedure> + 'static,
    {
        self.procedures.insert(id, Box::new(factory));
    }

    pub fn create(&self, id: &str) -> Result<Box<dyn Procedure>, ProcedureError> {
        let factory = self
            .procedures
            .get(id)
            .ok_or_else(|| ProcedureError::NotFound(id.to_string()))?;

        Ok(factory())
    }
}

pub struct Location(String);

pub trait Artifact {
    fn location(&self) -> Location;

    fn creator_rule(&self) -> Option<String>;
    fn consumer_rules(&self) -> Vec<String>;
}

pub enum ArtifactSelector {
    Inputs,
    Outputs,
    Both,
    None,
}

pub struct RuleOptions {
    pub ordered: ArtifactSelector,
    pub grouped: ArtifactSelector,
}

pub struct Rule {
    pub opts: RuleOptions,
    pub proc_id: ProcedureId,
    pub input_ids: Vec<String>,
    pub output_ids: Vec<String>,
}

pub struct Graph {
    rules: HashMap<String, Rule>,
    artifacts: HashMap<String, Box<dyn Artifact>>,
    // varables: Vec<Variables>,
}

impl Graph {
    pub fn new() -> Self {
        Self {
            rules: HashMap::new(),
            artifacts: HashMap::new(),
        }
    }
}

pub struct Plan {
    // The IDs of the Rules that are planned.
    pub rules: Vec<String>,
}

impl Plan {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }
}

pub enum ResolveMode {
    Terse,
    Full,
}

#[derive(Debug)]
pub enum PlanningError {}

impl Error for PlanningError {}

impl fmt::Display for PlanningError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "not implemented yet!")
    }
}

pub trait Resolver {
    fn resolve(&self, graph: &Graph, mode: ResolveMode) -> Result<Plan, PlanningError>;
}

// The TopologicalResolver does Kahn's algorithm to order the plan linearly.
pub struct TopologicalResolver {}

impl Resolver for TopologicalResolver {
    fn resolve(&self, graph: &Graph, mode: ResolveMode) -> Result<Plan, PlanningError> {
        let mut plan = Plan::new();
        let mut queue: VecDeque<String> = VecDeque::new();

        // A batch is a group of artifacts. When such group is used to process a rule,
        // they can be either inputs, or outputs.
        // For now, we are looking for batches that have no parents at all.
        'rule_loop: for (rule_id, rule) in graph.rules.iter() {
            for input_id in rule.input_ids.iter() {
                match graph.artifacts.get(input_id).unwrap().creator_rule() {
                    Some(_) => continue 'rule_loop,
                    None => continue,
                }
            }
            queue.push_back(rule_id.clone());
        }

        // Loop until the queue becomes empty.
        while !queue.is_empty() {
            let rule_id = queue.pop_front().unwrap();
            plan.rules.push(rule_id.clone());

            // For each output artifact of this rule, check their consumer rules to see
            // if they are ready to be scheduled.
            for output_id in graph.rules.get(&rule_id).unwrap().output_ids.iter() {
                'consumer_rule_loop: for consumer_rule_id in graph
                    .artifacts
                    .get(output_id)
                    .unwrap()
                    .consumer_rules()
                    .iter()
                {
                    for consumer_input_id in
                        graph.rules.get(consumer_rule_id).unwrap().input_ids.iter()
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
                    // Maybe fixme... Somehow, we might need to check first if we didn't already schedule the rule.
                    queue.push_back(consumer_rule_id.clone());
                }
            }
        }

        return Ok(plan);
    }
}

// #[derive(Debug)]
// pub enum ExecuteError {}
//
// impl Error for ExecuteError {}
//
// impl fmt::Display for ExecuteError {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         write!(f, "not implemented yet!")
//     }
// }
//
// pub trait Executor {
//     fn exec(&self, plan: &Plan) -> Result<Manifest, ExecuteError>; // ofc this will need to return errors or logs.
// }
//
// #[derive(Debug)]
// pub enum CacheError {}
//
// impl Error for CacheError {}
//
// impl fmt::Display for CacheError {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         write!(f, "not implemented yet!")
//     }
// }
//
//
// pub trait Cache {
//
//     // Artifacts
//     fn has_artifacts(&self, artifacts: &Vec<Box<dyn Artifact>>) -> bool;
//
//     fn store_artifacts(&self, artifacts: &Vec<Box<dyn Artifact>>) -> Option<CacheError>;
//
//     fn fetch_artifacts(&self, artifacts: &mut Vec<Box<dyn Artifact>>) -> Option<CacheError>;
//
//     // Graph state
//
//     fn primordial_rule_changed();
//
//     fn record_primordial_rule();
//
//     fn clear_primordial_rules();
//
//     fn get_plan();
//
//     fn clear_plan();
//
//     fn plan_rule();
// }
