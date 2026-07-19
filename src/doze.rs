use std::collections::HashMap;
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
        where F: Fn() -> Box<dyn Procedure> + 'static
    {
        self.procedures.insert(id, Box::new(factory));
    }

    pub fn create(&self, id: &str) -> Result<Box<dyn Procedure>, ProcedureError> {
        let factory = self
            .procedures
            .get(id)
            .ok_or_else(|| {
                ProcedureError::NotFound(id.to_string())
            })?;

        Ok(factory())
    }
}

pub struct Location(String);

pub trait Artifact {
    fn location(&self) -> Location;

    fn creator(&self) -> Box<Rule>;
    fn consumers(&self) -> Vec<Box<Rule>>;
}

pub enum ArtifactSelector {
    Inputs,
    Outputs,
    Both,
    None
}

pub struct RuleOptions {
    pub order: ArtifactSelector,
    pub grouped: ArtifactSelector,
}

pub struct Rule {
    pub opts: RuleOptions,
    pub proc_id: ProcedureId,
    pub inputs: Vec<Box<dyn Artifact>>,
    pub outputs: Vec<Box<dyn Artifact>>,
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

pub struct Plan {}

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

pub struct Manifest {}

#[derive(Debug)]
pub enum ExecuteError {}

impl Error for ExecuteError {}

impl fmt::Display for ExecuteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "not implemented yet!")
    }
}

pub trait Executor {
    fn exec(&self, plan: &Plan) -> Result<Manifest, ExecuteError>; // ofc this will need to return errors or logs.
}

#[derive(Debug)]
pub enum CacheError {}

impl Error for CacheError {}

impl fmt::Display for CacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "not implemented yet!")
    }
}


pub trait Cache {

    // Artifacts
    fn has_artifacts(&self, artifacts: &Vec<Box<dyn Artifact>>) -> bool;

    fn store_artifacts(&self, artifacts: &Vec<Box<dyn Artifact>>) -> Option<CacheError>;

    fn fetch_artifacts(&self, artifacts: &mut Vec<Box<dyn Artifact>>) -> Option<CacheError>;

    // Graph state

    fn primordial_rule_changed();

    fn record_primordial_rule();

    fn clear_primordial_rules();

    fn get_plan();

    fn clear_plan();

    fn plan_rule();
}
