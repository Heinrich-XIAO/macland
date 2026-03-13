use crate::adapter::AdapterManifest;
use serde::Deserialize;
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HostStatusEnvelope {
    status: String,
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
    let _ = fs::remove_file(&status_path);
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
        let status_payload = fs::read_to_string(&artifacts.status_path).map_err(|_| {
            format!(
                "host exited without writing status file {}",
                artifacts.status_path.display()
            )
        })?;
        let envelope: HostStatusEnvelope =
            serde_json::from_str(&status_payload).map_err(|err| err.to_string())?;
        if envelope.status == "host_started"
            || envelope.status == "child_started"
            || envelope.status == "child_exit:0"
        {
            Ok(())
        } else {
            Err(format!("host reported status {}", envelope.status))
        }
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
    use super::{HostLaunchArtifacts, HostSessionMode, create_launch_request, launch_host};
    use crate::adapter::{AdapterManifest, BuildSystem};
    use std::collections::BTreeMap;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};

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

    #[test]
    fn launch_host_requires_status_file() {
        let temp = std::env::temp_dir().join(format!("macland-host-launch-{}", std::process::id()));
        if temp.exists() {
            fs::remove_dir_all(&temp).unwrap();
        }
        fs::create_dir_all(&temp).unwrap();

        let host_binary = write_script(&temp.join("host-no-status.sh"), "#!/bin/sh\nexit 0\n");
        let artifacts = HostLaunchArtifacts {
            request_path: temp.join("request.json"),
            status_path: temp.join("status.json"),
        };
        fs::write(&artifacts.request_path, "{}").unwrap();
        let err = launch_host(&host_binary, &artifacts).unwrap_err();
        assert!(err.contains("without writing status file"));

        fs::remove_dir_all(&temp).unwrap();
    }

    #[test]
    fn launch_host_reports_failure_status() {
        let temp =
            std::env::temp_dir().join(format!("macland-host-launch-fail-{}", std::process::id()));
        if temp.exists() {
            fs::remove_dir_all(&temp).unwrap();
        }
        fs::create_dir_all(&temp).unwrap();

        let status_path = temp.join("status.json");
        let host_binary = write_script(
            &temp.join("host-fail-status.sh"),
            &format!(
                "#!/bin/sh\ncat <<'EOF' > \"{}\"\n{{\"status\":\"child_failed:test\"}}\nEOF\nexit 0\n",
                status_path.display()
            ),
        );
        let artifacts = HostLaunchArtifacts {
            request_path: temp.join("request.json"),
            status_path: status_path.clone(),
        };
        fs::write(&artifacts.request_path, "{}").unwrap();
        let err = launch_host(&host_binary, &artifacts).unwrap_err();
        assert!(err.contains("child_failed:test"));

        fs::remove_dir_all(&temp).unwrap();
    }

    fn write_script(path: &Path, contents: &str) -> PathBuf {
        fs::write(path, contents).unwrap();
        let mut permissions = fs::metadata(path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).unwrap();
        path.to_path_buf()
    }
}
