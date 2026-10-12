use std::fs;
use std::path::Path;

use doze::{Graph, ProcedureId, Registry};

use crate::{Dozefile, Error};

fn from_str(content: &str) -> Result<Dozefile, Error> {
    serde_saphyr::from_str(content).map_err(|e| Error::Parse(e.to_string()))
}

pub fn from_path(path: &Path) -> Result<Dozefile, Error> {
    let content = fs::read_to_string(path)
        .map_err(|e| Error::Read(path.display().to_string(), e.to_string()))?;
    from_str(&content)
}

pub fn build_graph(mut dozefile: Dozefile, registry: &Registry) -> Result<Graph, Error> {
    let mut graph = Graph::new();

    for rule_spec in dozefile.rules.drain(..) {
        graph.add_rule(
            rule_spec.inputs,
            rule_spec.outputs,
            rule_spec.input_dir,
            rule_spec.output_dir,
            ProcedureId(rule_spec.proc_id),
            registry,
        ).unwrap();
    }

    Ok(graph)
}
