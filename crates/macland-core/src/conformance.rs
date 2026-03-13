use crate::adapter::AdapterManifest;
use crate::host::{HostLaunchArtifacts, HostSessionMode, create_launch_request, launch_host};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConformanceReport {
    pub host_launched: bool,
    pub child_started: bool,
    pub child_exited_successfully: bool,
    pub status_file: PathBuf,
}

impl ConformanceReport {
    pub fn passed(&self) -> bool {
        self.host_launched && self.child_started && self.child_exited_successfully
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
    launch_host(host_binary, &artifacts)?;
    parse_status(&artifacts)
}

fn parse_status(artifacts: &HostLaunchArtifacts) -> Result<ConformanceReport, String> {
    let status = fs::read_to_string(&artifacts.status_path).map_err(|err| err.to_string())?;
    if let Ok(envelope) = serde_json::from_str::<StatusEnvelope>(&status) {
        return Ok(ConformanceReport {
            host_launched: true,
            child_started: envelope.status.contains("child_started"),
            child_exited_successfully: envelope.status.contains("child_exit:0"),
            status_file: artifacts.status_path.clone(),
        });
    }

    Ok(ConformanceReport {
        host_launched: true,
        child_started: status.contains("child_started"),
        child_exited_successfully: status.contains("child_exit:0"),
        status_file: artifacts.status_path.clone(),
    })
}

#[derive(Debug, Deserialize)]
struct StatusEnvelope {
    status: String,
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
