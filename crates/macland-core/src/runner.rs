use crate::adapter::{AdapterManifest, BuildSystem};
use crate::report::{ActionRecord, SupportReport, SupportTier, write_action_record};
use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};
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

    let mut process = Command::new(binary);
    process.args(args).current_dir(cwd);
    for (key, value) in effective_env(env_pairs) {
        process.env(key, value);
    }

    let status = process.status().map_err(|err| err.to_string())?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "command `{}` failed with status {}",
            command.join(" "),
            status
        ))
    }
}

pub fn effective_env(env_pairs: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    let mut merged = env_pairs.clone();
    if let Some(path) = merged_pkg_config_path(env_pairs.get("PKG_CONFIG_PATH")) {
        merged.insert("PKG_CONFIG_PATH".to_string(), path);
    }
    if let Some(path) = merged_prefix_path(env_pairs.get("CMAKE_PREFIX_PATH")) {
        merged.insert("CMAKE_PREFIX_PATH".to_string(), path);
    }
    if let Some(path) = merged_include_path(env_pairs.get("CPATH")) {
        merged.insert("CPATH".to_string(), path);
    }
    if let Some(flags) = merged_preprocessor_flags(env_pairs.get("CPPFLAGS")) {
        merged.insert("CPPFLAGS".to_string(), flags);
    }
    if let Some(flags) = merged_compile_flags(env_pairs.get("CFLAGS")) {
        merged.insert("CFLAGS".to_string(), flags.clone());
        merged.insert(
            "CXXFLAGS".to_string(),
            merged_compile_flags(env_pairs.get("CXXFLAGS")).unwrap_or(flags),
        );
    } else if let Some(flags) = merged_compile_flags(env_pairs.get("CXXFLAGS")) {
        merged.insert("CXXFLAGS".to_string(), flags);
    }
    if let Some(path) = merged_library_path(env_pairs.get("LIBRARY_PATH")) {
        merged.insert("LIBRARY_PATH".to_string(), path);
    }
    if let Some(flags) = merged_linker_flags(env_pairs.get("LDFLAGS")) {
        merged.insert("LDFLAGS".to_string(), flags);
    }
    if let Some(path) = merged_library_path(env_pairs.get("DYLD_FALLBACK_LIBRARY_PATH")) {
        merged.insert("DYLD_FALLBACK_LIBRARY_PATH".to_string(), path);
    }
    if let Some(path) = merged_path_env(env_pairs.get("PATH")) {
        merged.insert("PATH".to_string(), path);
    }
    merged
}

fn merged_pkg_config_path(override_value: Option<&String>) -> Option<String> {
    merge_path_list(
        env::var_os("PKG_CONFIG_PATH"),
        override_value,
        &[
            ".macland/sysroot/lib/pkgconfig",
            ".macland/sysroot/share/pkgconfig",
            "/opt/homebrew/lib/pkgconfig",
            "/opt/homebrew/share/pkgconfig",
            "/opt/homebrew/opt/epoll-shim/lib/pkgconfig",
            "/opt/homebrew/opt/jpeg/lib/pkgconfig",
            "/opt/homebrew/opt/libxkbcommon/lib/pkgconfig",
            "/opt/homebrew/opt/mesa/lib/pkgconfig",
            "/usr/local/lib/pkgconfig",
            "/usr/local/share/pkgconfig",
        ],
    )
}

fn merged_prefix_path(override_value: Option<&String>) -> Option<String> {
    merge_path_list(
        env::var_os("CMAKE_PREFIX_PATH"),
        override_value,
        &[
            ".macland/sysroot",
            "/opt/homebrew",
            "/opt/homebrew/opt/libxkbcommon",
            "/opt/homebrew/opt/mesa",
            "/usr/local",
            "/usr/local/opt/libxkbcommon",
            "/usr/local/opt/mesa",
        ],
    )
}

fn merged_include_path(override_value: Option<&String>) -> Option<String> {
    merge_path_list(
        env::var_os("CPATH"),
        override_value,
        &[
            ".macland/sysroot/include/libepoll-shim",
            ".macland/sysroot/include",
            "/opt/homebrew/include",
            "/opt/homebrew/opt/epoll-shim/include",
            "/opt/homebrew/opt/jpeg/include",
            "/opt/homebrew/opt/libxkbcommon/include",
            "/opt/homebrew/opt/mesa/include",
            "/usr/local/include",
            "/usr/local/opt/libxkbcommon/include",
            "/usr/local/opt/mesa/include",
        ],
    )
}

fn merged_library_path(override_value: Option<&String>) -> Option<String> {
    merge_path_list(
        env::var_os("LIBRARY_PATH"),
        override_value,
        &[
            ".macland/sysroot/lib",
            "/opt/homebrew/lib",
            "/opt/homebrew/opt/epoll-shim/lib",
            "/opt/homebrew/opt/jpeg/lib",
            "/opt/homebrew/opt/libxkbcommon/lib",
            "/opt/homebrew/opt/mesa/lib",
            "/usr/local/lib",
            "/usr/local/opt/libxkbcommon/lib",
            "/usr/local/opt/mesa/lib",
        ],
    )
}

fn merged_preprocessor_flags(override_value: Option<&String>) -> Option<String> {
    merge_flag_list(
        env::var_os("CPPFLAGS"),
        override_value,
        &[
            "-I.macland/sysroot/include/libepoll-shim",
            "-I.macland/sysroot/include",
            "-I/opt/homebrew/include",
            "-I/opt/homebrew/opt/epoll-shim/include",
            "-I/opt/homebrew/opt/jpeg/include",
            "-I/opt/homebrew/opt/libxkbcommon/include",
            "-I/opt/homebrew/opt/mesa/include",
            "-I/usr/local/include",
        ],
    )
}

fn merged_compile_flags(override_value: Option<&String>) -> Option<String> {
    merge_flag_list(
        None,
        override_value,
        &[
            "-I.macland/sysroot/include/libepoll-shim",
            "-I.macland/sysroot/include",
            "-I/opt/homebrew/include",
            "-I/opt/homebrew/opt/epoll-shim/include",
            "-I/opt/homebrew/opt/jpeg/include",
            "-I/opt/homebrew/opt/libxkbcommon/include",
            "-I/opt/homebrew/opt/mesa/include",
            "-I/usr/local/include",
        ],
    )
}

fn merged_linker_flags(override_value: Option<&String>) -> Option<String> {
    merge_flag_list(
        env::var_os("LDFLAGS"),
        override_value,
        &[
            "-lrt",
            "-L.macland/sysroot/lib",
            "-L/opt/homebrew/lib",
            "-L/opt/homebrew/opt/epoll-shim/lib",
            "-L/opt/homebrew/opt/jpeg/lib",
            "-L/opt/homebrew/opt/libxkbcommon/lib",
            "-L/opt/homebrew/opt/mesa/lib",
            "-L/usr/local/lib",
        ],
    )
}

fn merged_path_env(override_value: Option<&String>) -> Option<String> {
    merge_path_list(
        env::var_os("PATH"),
        override_value,
        &[
            ".macland/sysroot/bin",
            "/opt/homebrew/bin",
            "/usr/local/bin",
        ],
    )
}

fn merge_flag_list(
    inherited_value: Option<std::ffi::OsString>,
    override_value: Option<&String>,
    candidates: &[&str],
) -> Option<String> {
    let mut flags = Vec::new();

    for candidate in candidates {
        if let Some(resolved) = resolve_candidate_flag(candidate) {
            if !flags.iter().any(|flag| flag == &resolved) {
                flags.push(resolved);
            }
        }
    }

    if let Some(value) = inherited_value {
        flags.extend(
            value
                .to_string_lossy()
                .split_whitespace()
                .map(ToString::to_string),
        );
    }
    if let Some(value) = override_value {
        flags.extend(value.split_whitespace().map(ToString::to_string));
    }

    if flags.is_empty() {
        None
    } else {
        Some(flags.join(" "))
    }
}

fn merge_path_list(
    inherited_value: Option<std::ffi::OsString>,
    override_value: Option<&String>,
    candidates: &[&str],
) -> Option<String> {
    let mut paths = Vec::new();

    for candidate in candidates {
        if let Some(resolved) = resolve_candidate_path(candidate) {
            if !paths.iter().any(|path| path == &resolved) {
                paths.push(resolved);
            }
        }
    }

    if let Some(value) = inherited_value {
        paths.extend(env::split_paths(&value).map(|path| path.display().to_string()));
    }
    if let Some(value) = override_value {
        paths.extend(env::split_paths(value).map(|path| path.display().to_string()));
    }

    if paths.is_empty() {
        None
    } else {
        Some(paths.join(":"))
    }
}

fn resolve_candidate_flag(candidate: &str) -> Option<String> {
    if !candidate.starts_with("-I") && !candidate.starts_with("-L") {
        return Some(candidate.to_string());
    }

    let (prefix, path) = candidate.split_at(2);
    let resolved = resolve_candidate_path(path)?;
    Some(format!("{prefix}{resolved}"))
}

fn resolve_candidate_path(candidate: &str) -> Option<String> {
    let path = Path::new(candidate);
    if path.is_absolute() {
        return path.exists().then(|| path.display().to_string());
    }

    find_workspace_root()
        .map(|root| root.join(path))
        .filter(|path| path.exists())
        .map(|path| path.display().to_string())
}

fn find_workspace_root() -> Option<PathBuf> {
    let mut current = env::current_dir().ok()?;
    loop {
        if current.join("Cargo.toml").exists() && current.join("Package.swift").exists() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
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
    use super::{
        CommandPlan, execute_command_line, inspect_manifest, merged_include_path,
        merged_library_path, merged_linker_flags, merged_path_env, merged_pkg_config_path,
        merged_prefix_path,
    };
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

    #[test]
    fn augments_pkg_config_path_with_homebrew_roots() {
        let merged = merged_pkg_config_path(None).unwrap();
        assert!(merged.contains("/opt/homebrew/lib/pkgconfig"));
    }

    #[test]
    fn augments_cmake_prefix_path_with_homebrew_roots() {
        let merged = merged_prefix_path(None).unwrap();
        assert!(merged.contains("/opt/homebrew"));
    }

    #[test]
    fn augments_include_and_library_paths_with_homebrew_roots() {
        let include = merged_include_path(None).unwrap();
        let library = merged_library_path(None).unwrap();
        assert!(include.contains("/opt/homebrew/include"));
        assert!(library.contains("/opt/homebrew/lib"));
    }

    #[test]
    fn augments_path_with_homebrew_bin() {
        let merged = merged_path_env(None).unwrap();
        assert!(merged.contains("/opt/homebrew/bin"));
    }

    #[test]
    fn augments_linker_flags_with_managed_rt() {
        let merged = merged_linker_flags(None).unwrap();
        assert!(merged.contains("-lrt"));
    }
}
