pub mod yaml;

use serde::Deserialize;

#[derive(Debug)]
pub enum Error {
    Read(String, String),   // path, msg
    Parse(String),          // parser msg, includes code loc.
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dozefile {
    // vars: HashMap<String, String>,
    pub rules: Vec<RuleSpec>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleSpec {
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,

    #[serde(rename = "do")]
    pub proc_id: String,

    #[serde(rename = "in", default)]
    pub input_dir: String,
    #[serde(rename = "out", default)]
    pub output_dir: String,
}
