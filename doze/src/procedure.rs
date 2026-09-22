use std::collections::HashMap;
use std::error::Error;
use std::fmt;

use crate::Rule;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProcedureId(pub String);

#[derive(Debug)]
pub enum ProcedureError {
    NotFound(String),
    ExecFailed(String),
}

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

impl Error for ProcedureError {}

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

    pub fn with_builtins() -> Self {
        let mut registry = Self::new();
        for entry in inventory::iter::<ProcedureRegistration> {
            registry.register(entry.id.to_string(), entry.factory);
        }
        registry
    }

    pub fn contains(&self, id: &str) -> bool {
        self.procedures.contains_key(id)
    }

    pub fn register<F>(&mut self, id: String, factory: F)
    where
        F: Fn() -> Box<dyn Procedure> + 'static,
    {
        tracing::debug!(procedure_id = id, "registered built-in procedure");
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

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ProcedureRegistration {
    pub id: &'static str,
    pub factory: fn() -> Box<dyn Procedure>,
}

inventory::collect!(ProcedureRegistration);
