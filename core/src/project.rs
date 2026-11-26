use crate::{
    circuit::Circuit,
    cpu::{self, Architecture, Assembly},
    Result,
};
use serde::{Deserialize, Serialize};

pub const MAX_FILE: usize = 8 * 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub version: u8,
    pub name: String,
    pub architecture: Architecture,
    pub source: String,
    pub circuit: Circuit,
}
impl Project {
    pub fn parse(input: &str) -> Result<Self> {
        if input.len() > MAX_FILE {
            return Err("project exceeds 8 MiB".into());
        }
        let project: Self = serde_json::from_str(input).map_err(|e| e.to_string())?;
        if project.version != 1
            || project.name.trim().is_empty()
            || project.name.len() > 200
            || project.source.len() > 1_048_576
        {
            return Err("invalid project metadata or source size".into());
        }
        Ok(project)
    }
    pub fn validate(&self) -> Result<Assembly> {
        self.circuit.validate()?;
        cpu::assemble(&self.architecture, &self.source)
    }
}
pub fn reference(name: &str) -> Result<Project> {
    Project::parse(match name.to_ascii_lowercase().as_str() {
        "micro-8-cpu" => include_str!("../../examples/micro-8-cpu.json"),
        "micro-8" => include_str!("../../examples/micro-8.json"),
        "silicon-16" => include_str!("../../examples/silicon-16.json"),
        "silicon-32" => include_str!("../../examples/silicon-32.json"),
        _ => {
            return Err(
                "unknown reference project (micro-8, micro-8-cpu, silicon-16, silicon-32)".into(),
            )
        }
    })
}
