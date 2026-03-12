use crate::adapter::{AdapterManifest, BuildSystem};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub fn autodetect_manifest(id: &str, repo: &str, rev: &str, source_root: &Path) -> Option<AdapterManifest> {
    if source_root.join("Cargo.toml").exists() {
        return Some(cargo_manifest(id, repo, rev, source_root));
    }
    if source_root.join("meson.build").exists() {
        return Some(AdapterManifest {
            id: id.to_string(),
            repo: repo.to_string(),
            rev: rev.to_string(),
            build_system: BuildSystem::Meson,
            configure: vec!["meson".to_string(), "setup".to_string(), "build".to_string()],
            build: vec!["meson".to_string(), "compile".to_string(), "-C".to_string(), "build".to_string()],
            test: vec!["meson".to_string(), "test".to_string(), "-C".to_string(), "build".to_string()],
            entrypoint: Vec::new(),
            env: BTreeMap::new(),
            sdk_features: vec!["metal-fast-path".to_string()],
            protocol_expectations: vec!["xdg-shell".to_string()],
            patch_policy: "prefer-none".to_string(),
        });
    }
    if source_root.join("CMakeLists.txt").exists() {
        return Some(AdapterManifest {
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
            build: vec!["cmake".to_string(), "--build".to_string(), "build".to_string()],
            test: vec!["ctest".to_string(), "--test-dir".to_string(), "build".to_string()],
            entrypoint: Vec::new(),
            env: BTreeMap::new(),
            sdk_features: vec!["metal-fast-path".to_string()],
            protocol_expectations: vec!["xdg-shell".to_string()],
            patch_policy: "prefer-none".to_string(),
        });
    }
    if source_root.join("Makefile").exists() {
        return Some(AdapterManifest {
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
        });
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
}
