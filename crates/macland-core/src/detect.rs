use crate::adapter::{AdapterManifest, BuildSystem};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub fn autodetect_manifest(
    id: &str,
    repo: &str,
    rev: &str,
    source_root: &Path,
) -> Option<AdapterManifest> {
    if source_root.join("Cargo.toml").exists() {
        return Some(apply_known_overrides(cargo_manifest(
            id,
            repo,
            rev,
            source_root,
        )));
    }
    if source_root.join("meson.build").exists() {
        let entrypoint = read_meson_project_name(&source_root.join("meson.build"))
            .map(|name| vec![format!("./build/{name}")])
            .unwrap_or_default();
        return Some(apply_known_overrides(AdapterManifest {
            id: id.to_string(),
            repo: repo.to_string(),
            rev: rev.to_string(),
            build_system: BuildSystem::Meson,
            configure: vec![
                "meson".to_string(),
                "setup".to_string(),
                "build".to_string(),
                "--reconfigure".to_string(),
            ],
            build: vec![
                "meson".to_string(),
                "compile".to_string(),
                "-C".to_string(),
                "build".to_string(),
            ],
            test: vec![
                "meson".to_string(),
                "test".to_string(),
                "-C".to_string(),
                "build".to_string(),
            ],
            entrypoint,
            env: BTreeMap::new(),
            sdk_features: vec!["metal-fast-path".to_string()],
            protocol_expectations: vec!["xdg-shell".to_string()],
            patch_policy: "prefer-none".to_string(),
        }));
    }
    if source_root.join("CMakeLists.txt").exists() {
        return Some(apply_known_overrides(AdapterManifest {
            id: id.to_string(),
            repo: repo.to_string(),
            rev: rev.to_string(),
            build_system: BuildSystem::CMake,
            configure: vec![
                "cmake".to_string(),
                "-S".to_string(),
                ".".to_string(),
                "-B".to_string(),
                "build".to_string(),
            ],
            build: vec![
                "cmake".to_string(),
                "--build".to_string(),
                "build".to_string(),
            ],
            test: vec![
                "ctest".to_string(),
                "--test-dir".to_string(),
                "build".to_string(),
            ],
            entrypoint: Vec::new(),
            env: BTreeMap::new(),
            sdk_features: vec!["metal-fast-path".to_string()],
            protocol_expectations: vec!["xdg-shell".to_string()],
            patch_policy: "prefer-none".to_string(),
        }));
    }
    if source_root.join("Makefile").exists() {
        return Some(apply_known_overrides(AdapterManifest {
            id: id.to_string(),
            repo: repo.to_string(),
            rev: rev.to_string(),
            build_system: BuildSystem::Make,
            configure: Vec::new(),
            build: vec!["make".to_string()],
            test: vec!["make".to_string(), "test".to_string()],
            entrypoint: Vec::new(),
            env: BTreeMap::new(),
            sdk_features: vec!["metal-fast-path".to_string()],
            protocol_expectations: vec!["xdg-shell".to_string()],
            patch_policy: "prefer-none".to_string(),
        }));
    }
    None
}

fn cargo_manifest(id: &str, repo: &str, rev: &str, source_root: &Path) -> AdapterManifest {
    let entrypoint = match read_cargo_package_name(&source_root.join("Cargo.toml")) {
        Some(name) => vec![
            "cargo".to_string(),
            "run".to_string(),
            "--bin".to_string(),
            name,
        ],
        None => vec!["cargo".to_string(), "run".to_string()],
    };

    AdapterManifest {
        id: id.to_string(),
        repo: repo.to_string(),
        rev: rev.to_string(),
        build_system: BuildSystem::Cargo,
        configure: vec!["cargo".to_string(), "fetch".to_string()],
        build: vec!["cargo".to_string(), "build".to_string()],
        test: vec!["cargo".to_string(), "test".to_string()],
        entrypoint,
        env: BTreeMap::new(),
        sdk_features: vec!["metal-fast-path".to_string()],
        protocol_expectations: vec!["xdg-shell".to_string()],
        patch_policy: "prefer-none".to_string(),
    }
}

fn read_cargo_package_name(path: &Path) -> Option<String> {
    let contents = fs::read_to_string(path).ok()?;
    let mut in_package = false;
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_package = trimmed == "[package]";
            continue;
        }
        if in_package && trimmed.starts_with("name") {
            let (_, value) = trimmed.split_once('=')?;
            return Some(value.trim().trim_matches('"').to_string());
        }
    }
    None
}

fn read_meson_project_name(path: &Path) -> Option<String> {
    let contents = fs::read_to_string(path).ok()?;
    for line in contents.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("project(") {
            continue;
        }
        let after_open = trimmed.strip_prefix("project(")?;
        let first_arg = after_open.split(',').next()?.trim();
        return Some(first_arg.trim_matches('\'').trim_matches('"').to_string());
    }
    None
}

fn apply_known_overrides(mut manifest: AdapterManifest) -> AdapterManifest {
    let id_lower = manifest.id.to_ascii_lowercase();
    let repo_lower = manifest.repo.to_ascii_lowercase();

    if manifest.build_system == BuildSystem::CMake
        && (id_lower == "hyprland" || repo_lower.contains("hyprland"))
    {
        for flag in ["-DNO_XWAYLAND=ON", "-DNO_SYSTEMD=ON", "-DNO_UWSM=ON"] {
            if !manifest.configure.iter().any(|entry| entry == flag) {
                manifest.configure.push(flag.to_string());
            }
        }
    }

    if manifest.build_system == BuildSystem::Meson
        && (id_lower == "sway"
            || id_lower == "labwc"
            || repo_lower.contains("/sway")
            || repo_lower.contains("/labwc"))
    {
        for flag in [
            "-Dwlroots:backends=x11",
            "-Dwlroots:session=disabled",
            "-Dwlroots:libliftoff=disabled",
            "-Dwlroots:xwayland=disabled",
            "-Dwlroots:color-management=disabled",
        ] {
            if !manifest.configure.iter().any(|entry| entry == flag) {
                manifest.configure.push(flag.to_string());
            }
        }
    }

    manifest
}

#[cfg(test)]
mod tests {
    use super::autodetect_manifest;
    use crate::adapter::BuildSystem;
    use std::fs;

    #[test]
    fn detects_cargo_repo() {
        let root = std::env::temp_dir().join(format!("macland-detect-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"demo-compositor\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        let manifest = autodetect_manifest("demo", "https://example.com", "main", &root).unwrap();
        assert_eq!(manifest.build_system, BuildSystem::Cargo);
        assert_eq!(
            manifest.entrypoint,
            vec![
                "cargo".to_string(),
                "run".to_string(),
                "--bin".to_string(),
                "demo-compositor".to_string()
            ]
        );
    }

    #[test]
    fn detects_meson_repo() {
        let root =
            std::env::temp_dir().join(format!("macland-detect-meson-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("meson.build"),
            "project('meson-compositor', 'c')\n",
        )
        .unwrap();
        let manifest = autodetect_manifest("demo", "https://example.com", "main", &root).unwrap();
        assert_eq!(manifest.build_system, BuildSystem::Meson);
        assert_eq!(
            manifest.entrypoint,
            vec!["./build/meson-compositor".to_string()]
        );
    }

    #[test]
    fn applies_hyprland_overrides() {
        let root =
            std::env::temp_dir().join(format!("macland-detect-cmake-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("CMakeLists.txt"),
            "cmake_minimum_required(VERSION 3.20)\n",
        )
        .unwrap();

        let manifest = autodetect_manifest(
            "Hyprland",
            "https://github.com/hyprwm/Hyprland.git",
            "main",
            &root,
        )
        .unwrap();
        assert!(
            manifest
                .configure
                .iter()
                .any(|value| value == "-DNO_XWAYLAND=ON")
        );
        assert!(
            manifest
                .configure
                .iter()
                .any(|value| value == "-DNO_SYSTEMD=ON")
        );
        assert!(
            manifest
                .configure
                .iter()
                .any(|value| value == "-DNO_UWSM=ON")
        );
    }

    #[test]
    fn applies_wlroots_family_meson_overrides() {
        let root =
            std::env::temp_dir().join(format!("macland-detect-wlroots-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("meson.build"), "project('sway', 'c')\n").unwrap();

        let manifest =
            autodetect_manifest("sway", "https://github.com/swaywm/sway.git", "main", &root)
                .unwrap();
        assert!(
            manifest
                .configure
                .iter()
                .any(|value| value == "--reconfigure")
        );
        assert!(
            manifest
                .configure
                .iter()
                .any(|value| value == "-Dwlroots:backends=x11")
        );
        assert!(
            manifest
                .configure
                .iter()
                .any(|value| value == "-Dwlroots:libliftoff=disabled")
        );
        assert!(
            manifest
                .configure
                .iter()
                .any(|value| value == "-Dwlroots:xwayland=disabled")
        );
    }
}
