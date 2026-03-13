use crate::adapter::AdapterManifest;
use crate::backend::BackendCapabilities;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositorFamily {
    Wlroots,
    Weston,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShimAssessment {
    pub family: CompositorFamily,
    pub ready: bool,
    pub missing_sdk_features: Vec<String>,
    pub missing_protocols: Vec<String>,
    pub missing_backend_flags: Vec<String>,
}

impl ShimAssessment {
    pub fn summary(&self) -> &'static str {
        if self.ready { "ready" } else { "incomplete" }
    }
}

pub fn assess_manifest(
    manifest: &AdapterManifest,
    backend: &BackendCapabilities,
) -> ShimAssessment {
    let family = detect_family(manifest);
    let (sdk_requirements, protocol_requirements) = match family {
        CompositorFamily::Wlroots => (
            vec!["metal-fast-path", "seat-v1", "event-queue-v1"],
            vec!["xdg-shell", "layer-shell"],
        ),
        CompositorFamily::Weston => (vec!["metal-fast-path", "seat-v1"], vec!["xdg-shell"]),
        CompositorFamily::Custom => (Vec::new(), Vec::new()),
    };

    let missing_sdk_features = sdk_requirements
        .into_iter()
        .filter(|feature| {
            !manifest
                .sdk_features
                .iter()
                .any(|present| present == feature)
        })
        .map(str::to_string)
        .collect::<Vec<_>>();
    let missing_protocols = protocol_requirements
        .into_iter()
        .filter(|protocol| {
            !manifest
                .protocol_expectations
                .iter()
                .any(|present| present == protocol)
        })
        .map(str::to_string)
        .collect::<Vec<_>>();

    let mut missing_backend_flags = Vec::new();
    if matches!(family, CompositorFamily::Wlroots | CompositorFamily::Weston) {
        if !backend.supports_c_abi {
            missing_backend_flags.push("c-abi".to_string());
        }
        if !backend.supports_event_queue {
            missing_backend_flags.push("event-queue".to_string());
        }
        if !backend.supports_fullscreen_host {
            missing_backend_flags.push("fullscreen-host".to_string());
        }
    }

    let ready = missing_sdk_features.is_empty()
        && missing_protocols.is_empty()
        && missing_backend_flags.is_empty();

    ShimAssessment {
        family,
        ready,
        missing_sdk_features,
        missing_protocols,
        missing_backend_flags,
    }
}

pub fn detect_family(manifest: &AdapterManifest) -> CompositorFamily {
    let combined = format!("{} {}", manifest.id, manifest.repo).to_ascii_lowercase();
    if combined.contains("weston") || combined.contains("libweston") {
        return CompositorFamily::Weston;
    }
    if combined.contains("sway")
        || combined.contains("labwc")
        || combined.contains("wayfire")
        || manifest
            .protocol_expectations
            .iter()
            .any(|protocol| protocol == "layer-shell")
    {
        return CompositorFamily::Wlroots;
    }
    CompositorFamily::Custom
}

#[cfg(test)]
mod tests {
    use super::{CompositorFamily, assess_manifest, detect_family};
    use crate::adapter::{AdapterManifest, BuildSystem};
    use crate::backend::BackendCapabilities;
    use std::collections::BTreeMap;

    fn manifest(
        id: &str,
        repo: &str,
        sdk_features: &[&str],
        protocols: &[&str],
    ) -> AdapterManifest {
        AdapterManifest {
            id: id.to_string(),
            repo: repo.to_string(),
            rev: "main".to_string(),
            build_system: BuildSystem::Meson,
            configure: Vec::new(),
            build: Vec::new(),
            test: Vec::new(),
            entrypoint: Vec::new(),
            env: BTreeMap::new(),
            sdk_features: sdk_features.iter().map(|v| (*v).to_string()).collect(),
            protocol_expectations: protocols.iter().map(|v| (*v).to_string()).collect(),
            patch_policy: "prefer-none".to_string(),
        }
    }

    #[test]
    fn detects_wlroots_family() {
        let manifest = manifest(
            "labwc",
            "https://github.com/labwc/labwc.git",
            &["metal-fast-path"],
            &["xdg-shell", "layer-shell"],
        );
        assert_eq!(detect_family(&manifest), CompositorFamily::Wlroots);
    }

    #[test]
    fn assesses_missing_requirements() {
        let manifest = manifest(
            "weston",
            "https://gitlab.freedesktop.org/wayland/weston.git",
            &["metal-fast-path"],
            &["xdg-shell"],
        );
        let assessment = assess_manifest(&manifest, &BackendCapabilities::macos_defaults());
        assert_eq!(assessment.family, CompositorFamily::Weston);
        assert_eq!(assessment.missing_sdk_features, vec!["seat-v1".to_string()]);
        assert!(!assessment.ready);
    }

    #[test]
    fn custom_family_is_ready_by_default() {
        let manifest = manifest("demo", "https://example.com/demo", &[], &[]);
        let assessment = assess_manifest(&manifest, &BackendCapabilities::macos_defaults());
        assert_eq!(assessment.family, CompositorFamily::Custom);
        assert!(assessment.ready);
    }
}
