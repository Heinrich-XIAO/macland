use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SupportTier {
    Experimental,
    Tier1,
    Tier2,
    Tier3,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SupportReport {
    pub buildable: bool,
    pub upstream_tests_pass: bool,
    pub conformance_pass: bool,
    pub fullscreen_run_pass: bool,
    pub tier: SupportTier,
}

impl SupportReport {
    pub fn inspect_defaults() -> Self {
        Self {
            buildable: false,
            upstream_tests_pass: false,
            conformance_pass: false,
            fullscreen_run_pass: false,
            tier: SupportTier::Experimental,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionRecord {
    pub action: String,
    pub success: bool,
    pub command: Vec<String>,
}

pub fn write_action_record(root: &Path, record: &ActionRecord) -> Result<(), String> {
    fs::create_dir_all(root).map_err(|err| err.to_string())?;
    let path = root.join(format!("{}.json", record.action));
    let contents = serde_json::to_vec_pretty(record).map_err(|err| err.to_string())?;
    fs::write(path, contents).map_err(|err| err.to_string())
}

pub fn load_action_record(root: &Path, action: &str) -> Option<ActionRecord> {
    let path = root.join(format!("{}.json", action));
    let contents = fs::read(path).ok()?;
    serde_json::from_slice(&contents).ok()
}
