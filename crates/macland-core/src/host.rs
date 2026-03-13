use crate::adapter::AdapterManifest;
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum HostSessionMode {
    Fullscreen,
    WindowedDebug,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostLaunchRequest {
    pub mode: HostSessionMode,
    pub compositor_executable: Option<String>,
    pub compositor_arguments: Vec<String>,
    pub environment: BTreeMap<String, String>,
    pub permission_hints: Vec<String>,
    pub working_directory: Option<String>,
    pub status_file: Option<String>,
    pub auto_exit_after_child: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostLaunchArtifacts {
    pub request_path: PathBuf,
    pub status_path: PathBuf,
}

pub fn create_launch_request(
    manifest: &AdapterManifest,
    source_root: &Path,
    mode: HostSessionMode,
    artifacts_root: &Path,
) -> Result<HostLaunchArtifacts, String> {
    let (binary, args) = manifest
        .entrypoint
        .split_first()
        .ok_or_else(|| "entrypoint is empty".to_string())?;
    fs::create_dir_all(artifacts_root).map_err(|err| err.to_string())?;
    let request_path = artifacts_root.join("host-launch.json");
    let status_path = artifacts_root.join("host-status.txt");
    let request = HostLaunchRequest {
        mode,
        compositor_executable: Some(resolve_binary(source_root, binary).display().to_string()),
        compositor_arguments: args.to_vec(),
        environment: manifest.env.clone(),
        permission_hints: vec!["accessibility".to_string(), "inputMonitoring".to_string()],
        working_directory: Some(source_root.display().to_string()),
        status_file: Some(status_path.display().to_string()),
        auto_exit_after_child: true,
    };
    let json = serde_json::to_vec_pretty(&request).map_err(|err| err.to_string())?;
    fs::write(&request_path, json).map_err(|err| err.to_string())?;
    Ok(HostLaunchArtifacts {
        request_path,
        status_path,
    })
}

pub fn launch_host(host_binary: &Path, artifacts: &HostLaunchArtifacts) -> Result<(), String> {
    let status = Command::new(host_binary)
        .args([
            "--config",
            artifacts.request_path.to_string_lossy().as_ref(),
        ])
        .status()
        .map_err(|err| err.to_string())?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("host exited with status {status}"))
    }
}

fn resolve_binary(source_root: &Path, binary: &str) -> PathBuf {
    let path = PathBuf::from(binary);
    if path.is_absolute() || !binary.contains('/') {
        path
    } else {
        source_root.join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::{HostSessionMode, create_launch_request};
    use crate::adapter::{AdapterManifest, BuildSystem};
    use std::collections::BTreeMap;

    #[test]
    fn creates_launch_request_file() {
        let root =
            std::env::temp_dir().join(format!("macland-host-artifacts-{}", std::process::id()));
        let manifest = AdapterManifest {
            id: "fixture".to_string(),
            repo: "https://example.com".to_string(),
            rev: "main".to_string(),
            build_system: BuildSystem::Custom,
            configure: Vec::new(),
            build: vec!["/usr/bin/true".to_string()],
            test: vec!["/usr/bin/true".to_string()],
            entrypoint: vec!["bin/demo".to_string(), "--flag".to_string()],
            env: BTreeMap::from([(String::from("MACLAND_MODE"), String::from("1"))]),
            sdk_features: vec!["metal-fast-path".to_string()],
            protocol_expectations: vec!["xdg-shell".to_string()],
            patch_policy: "prefer-none".to_string(),
        };
        let artifacts = create_launch_request(
            &manifest,
            Path::new("/tmp/demo"),
            HostSessionMode::WindowedDebug,
            &root,
        )
        .unwrap();
        let contents = std::fs::read_to_string(artifacts.request_path).unwrap();
        assert!(contents.contains("\"windowedDebug\""));
        assert!(contents.contains("/tmp/demo/bin/demo"));
    }

    use std::path::Path;
}
