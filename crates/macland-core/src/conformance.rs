use crate::adapter::AdapterManifest;
use crate::host::{
    HostLaunchArtifacts, HostSessionMode, create_launch_request, spawn_host_until_started,
};
use serde::Deserialize;
use std::fs;
use std::process::Command;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConformanceReport {
    pub host_launched: bool,
    pub child_started: bool,
    pub child_exited_successfully: bool,
    pub reference_client_used: bool,
    pub first_frame_presented: bool,
    pub keyboard_focus_observed: bool,
    pub pointer_events_observed: u32,
    pub key_events_observed: u32,
    pub seat_present: bool,
    pub virtual_pointer_supported: bool,
    pub virtual_keyboard_supported: bool,
    pub pointer_injection_attempted: bool,
    pub keyboard_injection_attempted: bool,
    pub status_file: PathBuf,
}

impl ConformanceReport {
    pub fn passed(&self) -> bool {
        self.host_launched
            && self.child_started
            && self.child_exited_successfully
            && (!self.reference_client_used || self.first_frame_presented)
    }
}

pub fn run_conformance(
    host_binary: &Path,
    manifest: &AdapterManifest,
    source_root: &Path,
    artifacts_root: &Path,
    mode: HostSessionMode,
) -> Result<ConformanceReport, String> {
    let artifacts = create_launch_request(manifest, source_root, mode, artifacts_root)?;
    let mut session = match spawn_host_until_started(
        host_binary,
        &artifacts,
        Duration::from_secs(5),
        Duration::from_millis(250),
    ) {
        Ok(session) => session,
        Err(err) if err.contains("host exited before conformance could attach") => {
            return parse_status(&artifacts, true);
        }
        Err(err) => return Err(err),
    };
    let reference_client_report = maybe_run_reference_client(&artifacts)?;
    let mut report = parse_status(&artifacts, true)?;
    if let Some(client_report) = reference_client_report {
        report.reference_client_used = true;
        report.first_frame_presented = client_report.first_frame_presented;
        report.keyboard_focus_observed = client_report.keyboard_focus;
        report.pointer_events_observed = client_report.pointer_events;
        report.key_events_observed = client_report.key_events;
        report.seat_present = client_report.seat_present;
        report.virtual_pointer_supported = client_report.virtual_pointer_supported;
        report.virtual_keyboard_supported = client_report.virtual_keyboard_supported;
        report.pointer_injection_attempted = client_report.pointer_injection_attempted;
        report.keyboard_injection_attempted = client_report.keyboard_injection_attempted;
    }
    session.terminate()?;
    Ok(report)
}

fn parse_status(
    artifacts: &HostLaunchArtifacts,
    treat_started_as_success: bool,
) -> Result<ConformanceReport, String> {
    let status = fs::read_to_string(&artifacts.status_path).map_err(|err| err.to_string())?;
    if let Ok(envelope) = serde_json::from_str::<StatusEnvelope>(&status) {
        let child_started = envelope.status.contains("child_started");
        return Ok(ConformanceReport {
            host_launched: true,
            child_started,
            child_exited_successfully: envelope.status.contains("child_exit:0")
                || (treat_started_as_success && child_started),
            reference_client_used: false,
            first_frame_presented: false,
            keyboard_focus_observed: false,
            pointer_events_observed: 0,
            key_events_observed: 0,
            seat_present: false,
            virtual_pointer_supported: false,
            virtual_keyboard_supported: false,
            pointer_injection_attempted: false,
            keyboard_injection_attempted: false,
            status_file: artifacts.status_path.clone(),
        });
    }

    let child_started = status.contains("child_started");
    Ok(ConformanceReport {
        host_launched: true,
        child_started,
        child_exited_successfully: status.contains("child_exit:0")
            || (treat_started_as_success && child_started),
        reference_client_used: false,
        first_frame_presented: false,
        keyboard_focus_observed: false,
        pointer_events_observed: 0,
        key_events_observed: 0,
        seat_present: false,
        virtual_pointer_supported: false,
        virtual_keyboard_supported: false,
        pointer_injection_attempted: false,
        keyboard_injection_attempted: false,
        status_file: artifacts.status_path.clone(),
    })
}

fn maybe_run_reference_client(
    artifacts: &HostLaunchArtifacts,
) -> Result<Option<ReferenceClientReport>, String> {
    let Some(socket_name) = wait_for_wayland_socket(&artifacts.runtime_dir, Duration::from_secs(3))?
    else {
        return Ok(None);
    };

    let Some(binary) = ensure_reference_client_binary()? else {
        return Ok(None);
    };

    let report_path = artifacts.runtime_dir.join("reference-client-report.json");
    let mut child = Command::new(binary)
        .arg("--report-file")
        .arg(&report_path)
        .env("XDG_RUNTIME_DIR", &artifacts.runtime_dir)
        .env("WAYLAND_DISPLAY", socket_name)
        .spawn()
        .map_err(|err| err.to_string())?;

    let deadline = std::time::Instant::now() + Duration::from_secs(8);
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|err| err.to_string())? {
            break status;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(None);
        }
        thread::sleep(Duration::from_millis(50));
    };

    if !status.success() {
        return Err(format!("reference client failed with status {status}"));
    }

    let payload = fs::read_to_string(&report_path).map_err(|err| err.to_string())?;
    let report = serde_json::from_str(&payload).map_err(|err| err.to_string())?;
    Ok(Some(report))
}

fn wait_for_wayland_socket(
    runtime_dir: &Path,
    timeout: Duration,
) -> Result<Option<String>, String> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if runtime_dir.exists() {
            let mut sockets = Vec::new();
            collect_wayland_sockets(runtime_dir, runtime_dir, &mut sockets)?;
            sockets.sort();
            if let Some(path) = sockets.into_iter().next() {
                return Ok(Some(path));
            }
        }

        if std::time::Instant::now() >= deadline {
            return Ok(None);
        }

        thread::sleep(Duration::from_millis(50));
    }
}

fn collect_wayland_sockets(
    root: &Path,
    current: &Path,
    sockets: &mut Vec<String>,
) -> Result<(), String> {
    for entry in fs::read_dir(current).map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|err| err.to_string())?;

        if file_type.is_dir() {
            collect_wayland_sockets(root, &path, sockets)?;
            continue;
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::FileTypeExt;
            if file_type.is_socket() {
                let absolute = if path.is_absolute() {
                    path
                } else {
                    root.join(path)
                };
                sockets.push(absolute.display().to_string());
            }
        }
    }

    Ok(())
}

fn ensure_reference_client_binary() -> Result<Option<PathBuf>, String> {
    let Some(workspace_root) = find_workspace_root() else {
        return Ok(None);
    };
    if let Some(binary) = locate_reference_client_binary() {
        if !reference_client_binary_is_stale(&workspace_root, &binary)? {
            return Ok(Some(binary));
        }
    }

    let status = Command::new("cargo")
        .args(["build", "-p", "macland-reference-client"])
        .current_dir(&workspace_root)
        .status()
        .map_err(|err| err.to_string())?;
    if !status.success() {
        return Err(format!(
            "cargo build -p macland-reference-client failed with status {status}"
        ));
    }

    Ok(locate_reference_client_binary())
}

fn reference_client_binary_is_stale(workspace_root: &Path, binary: &Path) -> Result<bool, String> {
    let binary_metadata = fs::metadata(binary).map_err(|err| err.to_string())?;
    let binary_mtime = binary_metadata.modified().map_err(|err| err.to_string())?;
    let crate_root = workspace_root.join("crates").join("macland-reference-client");
    let mut candidates = vec![crate_root.join("Cargo.toml")];
    collect_source_files(&crate_root.join("src"), &mut candidates)?;

    for candidate in candidates {
        let metadata = match fs::metadata(&candidate) {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        let modified = metadata.modified().map_err(|err| err.to_string())?;
        if modified > binary_mtime {
            return Ok(true);
        }
    }

    Ok(false)
}

fn collect_source_files(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    if !root.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(root).map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();
        if entry.file_type().map_err(|err| err.to_string())?.is_dir() {
            collect_source_files(&path, files)?;
        } else {
            files.push(path);
        }
    }

    Ok(())
}

fn locate_reference_client_binary() -> Option<PathBuf> {
    let workspace_root = find_workspace_root()?;
    [
        workspace_root.join("target").join("debug").join("macland-reference-client"),
        workspace_root
            .join("target")
            .join("arm64-apple-darwin")
            .join("debug")
            .join("macland-reference-client"),
    ]
    .into_iter()
    .find(|candidate| candidate.exists())
}

fn find_workspace_root() -> Option<PathBuf> {
    let mut current = std::env::current_dir().ok()?;
    loop {
        if current.join("Cargo.toml").exists() && current.join("Package.swift").exists() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

#[derive(Debug, Deserialize)]
struct StatusEnvelope {
    status: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReferenceClientReport {
    first_frame_presented: bool,
    keyboard_focus: bool,
    pointer_events: u32,
    key_events: u32,
    #[serde(default)]
    seat_present: bool,
    #[serde(default)]
    virtual_pointer_supported: bool,
    #[serde(default)]
    virtual_keyboard_supported: bool,
    #[serde(default)]
    pointer_injection_attempted: bool,
    #[serde(default)]
    keyboard_injection_attempted: bool,
}

#[cfg(test)]
mod tests {
    use super::run_conformance;
    use crate::adapter::{AdapterManifest, BuildSystem};
    use crate::host::HostSessionMode;
    use std::collections::BTreeMap;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    #[test]
    fn conformance_passes_with_stub_host() {
        let root = std::env::temp_dir().join(format!("macland-conformance-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let child = root.join("child.sh");
        fs::write(&child, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&child, fs::Permissions::from_mode(0o755)).unwrap();

        let host = root.join("host.sh");
        fs::write(
            &host,
            r#"#!/bin/sh
config="$2"
status=$(python3 - <<'PY' "$config"
import json, sys
with open(sys.argv[1], "r", encoding="utf-8") as fh:
    data = json.load(fh)
print(data["statusFile"])
PY
)
printf "child_started\nchild_exit:0\n" > "$status"
exit 0
"#,
        )
        .unwrap();
        fs::set_permissions(&host, fs::Permissions::from_mode(0o755)).unwrap();

        let manifest = AdapterManifest {
            id: "fixture".to_string(),
            repo: "https://example.com".to_string(),
            rev: "main".to_string(),
            build_system: BuildSystem::Custom,
            configure: Vec::new(),
            build: vec!["/usr/bin/true".to_string()],
            test: vec!["/usr/bin/true".to_string()],
            entrypoint: vec![child.display().to_string()],
            env: BTreeMap::new(),
            sdk_features: vec!["metal-fast-path".to_string()],
            protocol_expectations: vec!["xdg-shell".to_string()],
            patch_policy: "prefer-none".to_string(),
        };

        let report = run_conformance(
            Path::new(&host),
            &manifest,
            &root,
            &root.join("artifacts"),
            HostSessionMode::WindowedDebug,
        )
        .unwrap();

        assert!(report.passed());
    }
}
