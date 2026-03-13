use macland_core::adapter::AdapterManifest;
use macland_core::bootstrap::{BootstrapPlan, execute_bootstrap};
use macland_core::conformance::run_conformance;
use macland_core::detect::autodetect_manifest;
use macland_core::doctor::DoctorReport;
use macland_core::host::{HostSessionMode, create_launch_request, smoke_launch_host};
use macland_core::repo::{RepoSpec, RepoWorkspace};
use macland_core::report::{
    ActionRecord, SupportReport, SupportTier, load_action_record, write_action_record,
};
use macland_core::runner::{CommandPlan, execute_recorded_command_line, inspect_manifest};
use macland_core::shim::assess_manifest;
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

fn main() {
    if let Err(err) = run(env::args().collect()) {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    let command = args.get(1).map(String::as_str).unwrap_or("help");
    let workspace = RepoWorkspace::new(env::current_dir().map_err(|err| err.to_string())?);

    match command {
        "doctor" => {
            print_doctor(&workspace, DoctorReport::gather());
            Ok(())
        }
        "bootstrap" => {
            let execute = args.iter().any(|arg| arg == "--execute");
            run_bootstrap(execute)
        }
        "repo" => handle_repo(&workspace, &args[2..]),
        "inspect" => {
            let repo_id = args.get(2).ok_or_else(|| "missing repo id".to_string())?;
            let manifest = load_manifest(&workspace, repo_id)?;
            let spec = workspace
                .load_repo_spec(repo_id)
                .unwrap_or_else(|_| RepoSpec::new(repo_id, "", None));
            let report = inspect_repo(&workspace, &spec, &manifest);
            let shim = assess_manifest(&manifest, &DoctorReport::gather().backend);
            println!("repo: {}", manifest.id);
            println!("buildable: {}", report.buildable);
            println!("upstream_tests_pass: {}", report.upstream_tests_pass);
            println!("conformance_pass: {}", report.conformance_pass);
            println!("fullscreen_run_pass: {}", report.fullscreen_run_pass);
            println!("tier: {:?}", report.tier);
            println!("shim.family: {:?}", shim.family);
            println!("shim.status: {}", shim.summary());
            println!(
                "shim.missing_sdk_features: {}",
                shim.missing_sdk_features.join(",")
            );
            println!(
                "shim.missing_protocols: {}",
                shim.missing_protocols.join(",")
            );
            println!(
                "shim.missing_backend_flags: {}",
                shim.missing_backend_flags.join(",")
            );
            Ok(())
        }
        "build" => {
            let repo_id = args.get(2).ok_or_else(|| "missing repo id".to_string())?;
            run_action(
                "build",
                &workspace,
                repo_id,
                args.iter().any(|arg| arg == "--execute"),
            )
        }
        "test" => {
            let repo_id = args.get(2).ok_or_else(|| "missing repo id".to_string())?;
            run_test_action(
                &workspace,
                repo_id,
                &args[3..],
                args.iter().any(|arg| arg == "--execute"),
            )
        }
        "run" => {
            let repo_id = args.get(2).ok_or_else(|| "missing repo id".to_string())?;
            run_run_action(
                &workspace,
                repo_id,
                &args[3..],
                args.iter().any(|arg| arg == "--execute"),
            )
        }
        _ => {
            print_help();
            Ok(())
        }
    }
}

fn handle_repo(workspace: &RepoWorkspace, args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("add") => {
            let url = args.get(1).ok_or_else(|| "missing git url".to_string())?;
            let rev = if args.get(2).map(String::as_str) == Some("--rev") {
                args.get(3).cloned()
            } else {
                None
            };
            let id = infer_repo_id(url);
            let spec = RepoSpec::new(&id, url, rev);
            workspace.ensure_root()?;
            let repo_root = workspace.repo_root(&spec);
            let source_root = workspace.source_root(&spec);
            fs::create_dir_all(&repo_root).map_err(|err| err.to_string())?;
            fs::create_dir_all(&source_root).map_err(|err| err.to_string())?;
            workspace.write_repo_spec(&spec)?;
            let manifest_path =
                if let Some(path) = workspace.seed_manifest_from_override(&spec, false)? {
                    path
                } else {
                    workspace.write_manifest(&spec, &RepoWorkspace::adapter_template(&spec))?
                };
            println!("registered repo: {}", spec.id);
            println!("repo root: {}", repo_root.display());
            println!("source root: {}", source_root.display());
            println!("adapter template: {}", manifest_path.display());
            Ok(())
        }
        Some("sync") => {
            let repo_id = args.get(1).ok_or_else(|| "missing repo id".to_string())?;
            let spec = workspace.load_repo_spec(repo_id)?;
            let source_root = workspace.source_root(&spec);
            if source_root.join(".git").exists() {
                run_git(&source_root, ["fetch", "--all", "--tags"])?;
                if let Some(ref rev) = spec.rev {
                    run_git(&source_root, ["checkout", rev.as_str()])?;
                }
                run_git(&source_root, ["pull", "--ff-only"])?;
            } else {
                let status = Command::new("git")
                    .args(["clone", &spec.url, source_root.to_string_lossy().as_ref()])
                    .status()
                    .map_err(|err| err.to_string())?;
                if !status.success() {
                    return Err(format!("git clone failed with status {status}"));
                }
                if let Some(ref rev) = spec.rev {
                    run_git(&source_root, ["checkout", rev.as_str()])?;
                }
            }
            sync_git_submodules(&source_root)?;
            let applied_patches = workspace.apply_override_patches(&spec)?;
            ensure_wlroots_subproject(&source_root)?;
            maybe_autodetect_manifest(workspace, &spec, &source_root)?;
            if let Some(path) = workspace.seed_manifest_from_override(&spec, true)? {
                println!("seeded adapter override: {}", path.display());
            }
            for patch in applied_patches {
                println!("applied override patch: {}", patch.display());
            }
            println!("synced repo: {repo_id}");
            Ok(())
        }
        _ => Err("usage: macland repo <add|sync> ...".to_string()),
    }
}

fn print_doctor(workspace: &RepoWorkspace, report: DoctorReport) {
    println!("host.macos={}", report.host.macos);
    println!("host.apple_silicon={}", report.host.apple_silicon);
    println!("backend.renderer={:?}", report.backend.renderer);
    println!(
        "backend.software_fallback={}",
        report.backend.supports_software_fallback
    );
    println!(
        "backend.fullscreen_host={}",
        report.backend.supports_fullscreen_host
    );
    println!(
        "backend.windowed_debug={}",
        report.backend.supports_windowed_debug
    );
    println!(
        "backend.single_display_session={}",
        report.backend.supports_single_display_session
    );
    println!(
        "backend.multi_display_session={}",
        report.backend.supports_multi_display_session
    );
    println!("backend.c_abi={}", report.backend.supports_c_abi);
    println!(
        "backend.event_queue={}",
        report.backend.supports_event_queue
    );
    println!(
        "backend.permissions={}",
        report.backend.permission_requirements.join(",")
    );
    if let Some(permissions) = probe_permissions(workspace.root()) {
        println!("permission.accessibility={}", permissions.accessibility);
        println!(
            "permission.inputMonitoring={}",
            permissions.input_monitoring
        );
        println!(
            "permission.screenRecording={}",
            permissions.screen_recording
        );
    }
    for tool in report.tools {
        println!("tool.{}={} ({})", tool.name, tool.found, tool.detail);
    }
    for dependency in report.native_dependencies {
        println!(
            "dep.{}={} ({})",
            dependency.name, dependency.found, dependency.detail
        );
    }
}

fn load_manifest(workspace: &RepoWorkspace, repo_id: &str) -> Result<AdapterManifest, String> {
    let spec = RepoSpec::new(repo_id, "", None);
    workspace.load_manifest(&spec)
}

fn run_action(
    action: &str,
    workspace: &RepoWorkspace,
    repo_id: &str,
    execute: bool,
) -> Result<(), String> {
    let manifest = load_manifest(workspace, repo_id)?;
    let plan = CommandPlan::for_manifest(&manifest);
    let spec = workspace
        .load_repo_spec(repo_id)
        .unwrap_or_else(|_| RepoSpec::new(repo_id, "", None));
    let source_root = workspace.source_root(&spec);
    let repo_root = if source_root.exists() {
        source_root
    } else {
        workspace.repo_root(&spec)
    };
    let reports_root = workspace.artifacts_root(&spec).join("reports");

    let line = match action {
        "build" => plan.build.clone(),
        "test" => plan.test.clone(),
        "run" => plan.run.clone(),
        _ => Vec::new(),
    };

    println!("repo: {}", manifest.id);
    println!("action: {action}");
    if action == "build" && !plan.configure.is_empty() {
        println!("configure_command: {}", plan.configure.join(" "));
    }
    println!("command: {}", line.join(" "));
    println!("cwd: {}", repo_root.display());
    println!(
        "upstream_test_hint: {}",
        CommandPlan::upstream_test_hint(manifest.build_system)
    );
    if execute {
        if action == "build" && !plan.configure.is_empty() {
            execute_recorded_command_line(
                "configure",
                &repo_root,
                &plan.configure,
                &manifest.env,
                &reports_root,
            )?;
            println!("configure_status: success");
        }
        execute_recorded_command_line(action, &repo_root, &line, &manifest.env, &reports_root)?;
        println!("status: success");
    }
    Ok(())
}

fn run_test_action(
    workspace: &RepoWorkspace,
    repo_id: &str,
    args: &[String],
    execute: bool,
) -> Result<(), String> {
    let manifest = load_manifest(workspace, repo_id)?;
    let plan = CommandPlan::for_manifest(&manifest);
    let spec = workspace
        .load_repo_spec(repo_id)
        .unwrap_or_else(|_| RepoSpec::new(repo_id, "", None));
    let source_root = workspace.source_root(&spec);
    let run_upstream = !args.iter().any(|arg| arg == "--conformance")
        || args.iter().any(|arg| arg == "--upstream");
    let run_conformance_checks = !args.iter().any(|arg| arg == "--upstream")
        || args.iter().any(|arg| arg == "--conformance");

    println!("repo: {}", manifest.id);
    println!("action: test");
    println!("cwd: {}", source_root.display());
    println!("upstream_command: {}", plan.test.join(" "));
    println!("run_upstream: {}", run_upstream);
    println!("run_conformance: {}", run_conformance_checks);
    let reports_root = workspace.artifacts_root(&spec).join("reports");

    if execute && run_upstream {
        execute_recorded_command_line(
            "test",
            &source_root,
            &plan.test,
            &manifest.env,
            &reports_root,
        )?;
        println!("upstream_status: success");
    }

    if run_conformance_checks {
        let host_binary = locate_host_binary(workspace.root())?;
        let report = if execute {
            run_conformance(
                &host_binary,
                &manifest,
                &source_root,
                &workspace.artifacts_root(&spec).join("conformance"),
                HostSessionMode::WindowedDebug,
            )?
        } else {
            let artifacts = create_launch_request(
                &manifest,
                &source_root,
                HostSessionMode::WindowedDebug,
                &workspace.artifacts_root(&spec).join("conformance"),
            )?;
            println!(
                "conformance_launch_request: {}",
                artifacts.request_path.display()
            );
            return Ok(());
        };
        if execute {
            write_action_record(
                &reports_root,
                &ActionRecord {
                    action: "conformance".to_string(),
                    success: report.passed(),
                    command: vec![
                        host_binary.display().to_string(),
                        "--config".to_string(),
                        workspace
                            .artifacts_root(&spec)
                            .join("conformance")
                            .join("host-launch.json")
                            .display()
                            .to_string(),
                    ],
                },
            )?;
        }
        println!("conformance_status_file: {}", report.status_file.display());
        println!("conformance_reference_client_used: {}", report.reference_client_used);
        println!("conformance_first_frame_presented: {}", report.first_frame_presented);
        println!(
            "conformance_keyboard_focus_observed: {}",
            report.keyboard_focus_observed
        );
        println!(
            "conformance_pointer_events_observed: {}",
            report.pointer_events_observed
        );
        println!(
            "conformance_key_events_observed: {}",
            report.key_events_observed
        );
        println!("conformance_seat_present: {}", report.seat_present);
        println!(
            "conformance_virtual_pointer_supported: {}",
            report.virtual_pointer_supported
        );
        println!(
            "conformance_virtual_keyboard_supported: {}",
            report.virtual_keyboard_supported
        );
        println!(
            "conformance_pointer_injection_attempted: {}",
            report.pointer_injection_attempted
        );
        println!(
            "conformance_keyboard_injection_attempted: {}",
            report.keyboard_injection_attempted
        );
        println!("conformance_passed: {}", report.passed());
    }

    Ok(())
}

fn run_run_action(
    workspace: &RepoWorkspace,
    repo_id: &str,
    args: &[String],
    execute: bool,
) -> Result<(), String> {
    let manifest = load_manifest(workspace, repo_id)?;
    let spec = workspace
        .load_repo_spec(repo_id)
        .unwrap_or_else(|_| RepoSpec::new(repo_id, "", None));
    let source_root = workspace.source_root(&spec);
    let mode = if args.iter().any(|arg| arg == "--fullscreen") {
        HostSessionMode::Fullscreen
    } else {
        HostSessionMode::WindowedDebug
    };
    let artifacts = create_launch_request(
        &manifest,
        &source_root,
        mode,
        &workspace.artifacts_root(&spec).join("run"),
    )?;
    let host_binary = locate_host_binary(workspace.root())?;

    println!("repo: {}", manifest.id);
    println!("action: run");
    println!("host_binary: {}", host_binary.display());
    println!("launch_request: {}", artifacts.request_path.display());
    println!("status_file: {}", artifacts.status_path.display());
    println!("mode: {:?}", mode);

    if execute {
        smoke_launch_host(
            &host_binary,
            &artifacts,
            Duration::from_secs(5),
            Duration::from_millis(750),
        )?;
        write_action_record(
            &workspace.artifacts_root(&spec).join("reports"),
            &ActionRecord {
                action: "run".to_string(),
                success: true,
                command: vec![
                    host_binary.display().to_string(),
                    "--config".to_string(),
                    artifacts.request_path.display().to_string(),
                ],
            },
        )?;
        println!("status: success");
    }
    Ok(())
}

fn infer_repo_id(url: &str) -> String {
    PathBuf::from(url)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("repo")
        .to_string()
}

fn print_help() {
    println!("macland commands:");
    println!("  doctor");
    println!("  bootstrap [--execute]");
    println!("  repo add <git-url> [--rev <commit>]");
    println!("  repo sync <repo-id>");
    println!("  inspect <repo-id>");
    println!("  build <repo-id> [--execute]");
    println!("  test <repo-id> [--upstream|--conformance] [--execute]");
    println!("  run <repo-id> [--fullscreen|--windowed-debug] [--execute]");
}

fn locate_host_binary(workspace_root: &Path) -> Result<PathBuf, String> {
    if let Ok(path) = env::var("MACLAND_HOST_BINARY") {
        return Ok(PathBuf::from(path));
    }
    if let Some(binary) = find_swiftpm_binary(workspace_root, "macland-host") {
        Ok(binary)
    } else {
        Err(
            "macland-host binary is missing; run `swift build` first or set MACLAND_HOST_BINARY"
                .to_string(),
        )
    }
}

fn locate_permissions_binary(workspace_root: &Path) -> Option<PathBuf> {
    find_swiftpm_binary(workspace_root, "macland-permissions")
}

fn run_bootstrap(execute: bool) -> Result<(), String> {
    let report = DoctorReport::gather();
    let plan = BootstrapPlan::from_doctor(&report);
    if plan.is_empty() {
        println!("bootstrap: no missing managed tools");
        if execute {
            execute_bootstrap(&plan)?;
            println!("bootstrap_status: success");
        }
        return Ok(());
    }

    println!("bootstrap_packages: {}", plan.packages.join(" "));
    println!(
        "bootstrap_workspace_shims: {}",
        plan.workspace_shims.join(" ")
    );
    if execute {
        execute_bootstrap(&plan)?;
        println!("bootstrap_status: success");
    }
    Ok(())
}

fn run_git<const N: usize>(cwd: &Path, args: [&str; N]) -> Result<(), String> {
    let status = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .status()
        .map_err(|err| err.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "git {} failed with status {status}",
            args.join(" ")
        ))
    }
}

fn sync_git_submodules(source_root: &Path) -> Result<(), String> {
    if !source_root.join(".gitmodules").exists() {
        return Ok(());
    }

    let status = Command::new("git")
        .args([
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "update",
            "--init",
            "--recursive",
        ])
        .current_dir(source_root)
        .status()
        .map_err(|err| err.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("git submodule update failed with status {status}"))
    }
}

fn ensure_wlroots_subproject(source_root: &Path) -> Result<(), String> {
    let meson_build = source_root.join("meson.build");
    if !meson_build.exists() {
        return Ok(());
    }

    let contents = fs::read_to_string(&meson_build).map_err(|err| err.to_string())?;
    if !contents.contains("wlroots") {
        return Ok(());
    }

    let wlroots_root = source_root.join("subprojects").join("wlroots");
    if !wlroots_root.exists() {
        let requested_series =
            detect_wlroots_series(&contents).unwrap_or_else(|| "0.20".to_string());
        let reference = resolve_wlroots_ref(&requested_series)?;

        fs::create_dir_all(
            wlroots_root
                .parent()
                .ok_or_else(|| "invalid wlroots subproject path".to_string())?,
        )
        .map_err(|err| err.to_string())?;

        let status = Command::new("git")
            .args([
                "clone",
                "--depth",
                "1",
                "--branch",
                &reference,
                "https://gitlab.freedesktop.org/wlroots/wlroots.git",
                wlroots_root.to_string_lossy().as_ref(),
            ])
            .status()
            .map_err(|err| err.to_string())?;
        if !status.success() {
            return Err(format!("wlroots clone failed with status {status}"));
        }
    }

    ensure_wlroots_redirect_wraps(source_root)
}

fn ensure_wlroots_redirect_wraps(source_root: &Path) -> Result<(), String> {
    let subprojects_root = source_root.join("subprojects");
    if !subprojects_root.join("wlroots").exists() {
        return Ok(());
    }

    for (name, target) in [
        ("wayland.wrap", "wlroots/subprojects/wayland.wrap"),
        ("libdrm.wrap", "wlroots/subprojects/libdrm.wrap"),
        (
            "libdisplay-info.wrap",
            "wlroots/subprojects/libdisplay-info.wrap",
        ),
        ("libliftoff.wrap", "wlroots/subprojects/libliftoff.wrap"),
    ] {
        let redirect_path = subprojects_root.join(name);
        if redirect_path.exists() {
            continue;
        }

        let target_path = subprojects_root.join(target);
        if !target_path.exists() {
            continue;
        }

        fs::write(
            &redirect_path,
            format!("[wrap-redirect]\nfilename = {target}\n"),
        )
        .map_err(|err| err.to_string())?;
    }

    Ok(())
}

fn detect_wlroots_series(contents: &str) -> Option<String> {
    for marker in ["wlroots-0.", ">=0."] {
        if let Some(index) = contents.find(marker) {
            let suffix = &contents[index + marker.len()..];
            let mut digits = String::new();
            for character in suffix.chars() {
                if character.is_ascii_digit() || character == '.' {
                    digits.push(character);
                } else {
                    break;
                }
            }
            if marker == "wlroots-0." {
                if !digits.is_empty() {
                    return Some(format!("0.{digits}"));
                }
            } else if !digits.is_empty() {
                if let Some(minor) = digits.split('.').next() {
                    return Some(format!("0.{minor}"));
                }
            }
        }
    }
    None
}

fn resolve_wlroots_ref(series: &str) -> Result<String, String> {
    let output = Command::new("git")
        .args([
            "ls-remote",
            "--tags",
            "--refs",
            "https://gitlab.freedesktop.org/wlroots/wlroots.git",
            &format!("refs/tags/{series}*"),
        ])
        .output()
        .map_err(|err| err.to_string())?;
    if !output.status.success() {
        return Err(format!("failed to resolve wlroots refs for {series}"));
    }

    let mut refs = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.split('\t').nth(1))
        .filter_map(|reference| reference.rsplit('/').next())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    refs.sort();
    refs.into_iter()
        .rev()
        .find(|reference| reference.starts_with(series))
        .ok_or_else(|| format!("no wlroots tag found for {series}"))
}

fn maybe_autodetect_manifest(
    workspace: &RepoWorkspace,
    spec: &RepoSpec,
    source_root: &Path,
) -> Result<(), String> {
    let current = workspace.load_manifest(spec)?;
    let is_template = current.build_system == macland_core::adapter::BuildSystem::Custom
        && current.build.is_empty()
        && current.test.is_empty()
        && current.entrypoint.is_empty();
    if !is_template {
        return Ok(());
    }

    let rev = spec.rev.clone().unwrap_or_else(|| "main".to_string());
    if let Some(detected) = autodetect_manifest(&spec.id, &spec.url, &rev, source_root) {
        let contents = render_manifest(&detected);
        workspace.write_manifest(spec, &contents)?;
    }
    Ok(())
}

fn render_manifest(manifest: &AdapterManifest) -> String {
    let mut output = String::new();
    output.push_str(&format!("id = {:?}\n", manifest.id));
    output.push_str(&format!("repo = {:?}\n", manifest.repo));
    output.push_str(&format!("rev = {:?}\n", manifest.rev));
    output.push_str(&format!(
        "build_system = {:?}\n",
        format_build_system(manifest.build_system)
    ));
    output.push_str(&format!(
        "configure = {}\n",
        format_array(&manifest.configure)
    ));
    output.push_str(&format!("build = {}\n", format_array(&manifest.build)));
    output.push_str(&format!("test = {}\n", format_array(&manifest.test)));
    output.push_str(&format!(
        "entrypoint = {}\n",
        format_array(&manifest.entrypoint)
    ));
    output.push_str(&format!("patch_policy = {:?}\n", manifest.patch_policy));
    output.push_str(&format!(
        "sdk_features = {}\n",
        format_array(&manifest.sdk_features)
    ));
    output.push_str(&format!(
        "protocol_expectations = {}\n\n[env]\n",
        format_array(&manifest.protocol_expectations)
    ));
    for (key, value) in &manifest.env {
        output.push_str(&format!("{key} = {:?}\n", value));
    }
    output
}

fn format_array(values: &[String]) -> String {
    let items = values
        .iter()
        .map(|value| format!("{value:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{items}]")
}

fn format_build_system(system: macland_core::adapter::BuildSystem) -> &'static str {
    match system {
        macland_core::adapter::BuildSystem::Meson => "meson",
        macland_core::adapter::BuildSystem::CMake => "cmake",
        macland_core::adapter::BuildSystem::Cargo => "cargo",
        macland_core::adapter::BuildSystem::Autotools => "autotools",
        macland_core::adapter::BuildSystem::Make => "make",
        macland_core::adapter::BuildSystem::Custom => "custom",
    }
}

#[derive(Debug, Deserialize)]
struct PermissionProbeOutput {
    states: std::collections::BTreeMap<String, String>,
}

#[cfg(test)]
mod sync_tests {
    use super::{detect_wlroots_series, ensure_wlroots_redirect_wraps};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn detects_wlroots_series_from_dependency_name() {
        assert_eq!(
            detect_wlroots_series("wlroots = dependency('wlroots-0.20', fallback: 'wlroots')"),
            Some("0.20".to_string())
        );
    }

    #[test]
    fn detects_wlroots_series_from_version_constraint() {
        assert_eq!(
            detect_wlroots_series("wlroots_version = ['>=0.19.0', '<0.20.0']"),
            Some("0.19".to_string())
        );
    }

    #[test]
    fn seeds_redirect_wraps_from_wlroots_subproject() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("macland-sync-tests-{suffix}"));
        let source_root = root.join("source");
        let wlroots_subprojects = source_root
            .join("subprojects")
            .join("wlroots")
            .join("subprojects");
        fs::create_dir_all(&wlroots_subprojects).unwrap();
        fs::write(wlroots_subprojects.join("wayland.wrap"), "[wrap-git]\n").unwrap();
        fs::write(wlroots_subprojects.join("libdrm.wrap"), "[wrap-git]\n").unwrap();

        ensure_wlroots_redirect_wraps(&source_root).unwrap();

        assert_eq!(
            fs::read_to_string(source_root.join("subprojects").join("wayland.wrap")).unwrap(),
            "[wrap-redirect]\nfilename = wlroots/subprojects/wayland.wrap\n"
        );
        assert_eq!(
            fs::read_to_string(source_root.join("subprojects").join("libdrm.wrap")).unwrap(),
            "[wrap-redirect]\nfilename = wlroots/subprojects/libdrm.wrap\n"
        );
    }
}

#[derive(Debug)]
struct PermissionLines {
    accessibility: String,
    input_monitoring: String,
    screen_recording: String,
}

fn probe_permissions(workspace_root: &Path) -> Option<PermissionLines> {
    let binary = locate_permissions_binary(workspace_root)?;
    let output = Command::new(binary).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let parsed: PermissionProbeOutput = serde_json::from_slice(&output.stdout).ok()?;
    Some(PermissionLines {
        accessibility: parsed
            .states
            .get("accessibility")
            .cloned()
            .unwrap_or_else(|| "unknown".to_string()),
        input_monitoring: parsed
            .states
            .get("inputMonitoring")
            .cloned()
            .unwrap_or_else(|| "unknown".to_string()),
        screen_recording: parsed
            .states
            .get("screenRecording")
            .cloned()
            .unwrap_or_else(|| "unknown".to_string()),
    })
}

fn find_swiftpm_binary(workspace_root: &Path, name: &str) -> Option<PathBuf> {
    let candidates = [
        workspace_root.join(".build").join("debug").join(name),
        workspace_root
            .join(".build")
            .join("arm64-apple-macosx")
            .join("debug")
            .join(name),
    ];

    candidates.into_iter().find(|candidate| candidate.exists())
}

fn inspect_repo(
    workspace: &RepoWorkspace,
    spec: &RepoSpec,
    manifest: &AdapterManifest,
) -> SupportReport {
    let mut report = inspect_manifest(manifest);
    let reports_root = workspace.artifacts_root(spec).join("reports");
    report.upstream_tests_pass = load_action_record(&reports_root, "test")
        .map(|record| record.success)
        .unwrap_or(false);
    report.conformance_pass = load_action_record(&reports_root, "conformance")
        .map(|record| record.success)
        .unwrap_or(false);
    report.fullscreen_run_pass = load_action_record(&reports_root, "run")
        .map(|record| record.success)
        .unwrap_or(false);
    if load_action_record(&reports_root, "build")
        .map(|record| record.success)
        .unwrap_or(false)
    {
        report.buildable = true;
    }

    report.tier = if report.fullscreen_run_pass && report.conformance_pass {
        SupportTier::Tier1
    } else if report.buildable {
        SupportTier::Experimental
    } else {
        SupportTier::Experimental
    };

    report
}
