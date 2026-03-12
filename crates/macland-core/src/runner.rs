use crate::adapter::{AdapterManifest, BuildSystem};
use crate::report::{ActionRecord, SupportReport, SupportTier, write_action_record};
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandPlan {
    pub configure: Vec<String>,
    pub build: Vec<String>,
    pub test: Vec<String>,
    pub run: Vec<String>,
}

impl CommandPlan {
    pub fn for_manifest(manifest: &AdapterManifest) -> Self {
        Self {
            configure: manifest.configure.clone(),
            build: manifest.build.clone(),
            test: manifest.test.clone(),
            run: manifest.entrypoint.clone(),
        }
    }

    pub fn upstream_test_hint(build_system: BuildSystem) -> &'static str {
        match build_system {
            BuildSystem::Meson => "meson test",
            BuildSystem::CMake => "ctest",
            BuildSystem::Cargo => "cargo test",
            BuildSystem::Autotools | BuildSystem::Make => "make test",
            BuildSystem::Custom => "adapter-defined",
        }
    }
}

pub fn inspect_manifest(manifest: &AdapterManifest) -> SupportReport {
    let buildable = !manifest.build.is_empty() && !manifest.entrypoint.is_empty();
    let tier = if buildable {
        SupportTier::Tier1
    } else {
        SupportTier::Experimental
    };

    SupportReport {
        buildable,
        upstream_tests_pass: false,
        conformance_pass: false,
        fullscreen_run_pass: false,
        tier,
    }
}

pub fn spawn_child(binary: &str, args: &[String]) -> Result<(), String> {
    let status = Command::new(binary)
        .args(args)
        .status()
        .map_err(|err| err.to_string())?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("child exited with status {status}"))
    }
}

pub fn execute_command_line(
    cwd: &Path,
    command: &[String],
    env_pairs: &BTreeMap<String, String>,
) -> Result<(), String> {
    let (binary, args) = command
        .split_first()
        .ok_or_else(|| "empty command".to_string())?;

    let status = Command::new(binary)
        .args(args)
        .current_dir(cwd)
        .envs(env_pairs)
        .status()
        .map_err(|err| err.to_string())?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("command `{}` failed with status {}", command.join(" "), status))
    }
}

pub fn execute_recorded_command_line(
    action: &str,
    cwd: &Path,
    command: &[String],
    env_pairs: &BTreeMap<String, String>,
    reports_root: &Path,
) -> Result<(), String> {
    let result = execute_command_line(cwd, command, env_pairs);
    let success = result.is_ok();
    write_action_record(
        reports_root,
        &ActionRecord {
            action: action.to_string(),
            success,
            command: command.to_vec(),
        },
    )?;
    result
}

#[cfg(test)]
mod tests {
    use super::{CommandPlan, execute_command_line, inspect_manifest};
    use crate::adapter::{AdapterManifest, BuildSystem};
    use std::collections::BTreeMap;

    #[test]
    fn plans_commands() {
        let manifest = AdapterManifest {
            id: "sample".to_string(),
            repo: "https://example.com".to_string(),
            rev: "main".to_string(),
            build_system: BuildSystem::Cargo,
            configure: vec!["cargo".to_string(), "fetch".to_string()],
            build: vec!["cargo".to_string(), "build".to_string()],
            test: vec!["cargo".to_string(), "test".to_string()],
            entrypoint: vec!["cargo".to_string(), "run".to_string()],
            env: BTreeMap::new(),
            sdk_features: vec!["metal-fast-path".to_string()],
            protocol_expectations: vec!["xdg-shell".to_string()],
            patch_policy: "prefer-none".to_string(),
        };

        let plan = CommandPlan::for_manifest(&manifest);
        assert_eq!(plan.build, vec!["cargo".to_string(), "build".to_string()]);

        let report = inspect_manifest(&manifest);
        assert!(report.buildable);
    }

    #[test]
    fn executes_simple_command() {
        let cwd = std::env::current_dir().unwrap();
        execute_command_line(&cwd, &["/usr/bin/true".to_string()], &BTreeMap::new()).unwrap();
    }
}
