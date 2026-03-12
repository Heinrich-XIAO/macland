use macland_core::adapter::AdapterManifest;
use macland_core::doctor::DoctorReport;
use macland_core::repo::{RepoSpec, RepoWorkspace};
use macland_core::runner::{execute_command_line, inspect_manifest, CommandPlan};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

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
            print_doctor(DoctorReport::gather());
            Ok(())
        }
        "repo" => handle_repo(&workspace, &args[2..]),
        "inspect" => {
            let repo_id = args.get(2).ok_or_else(|| "missing repo id".to_string())?;
            let manifest = load_manifest(&workspace, repo_id)?;
            let report = inspect_manifest(&manifest);
            println!("repo: {}", manifest.id);
            println!("buildable: {}", report.buildable);
            println!("upstream_tests_pass: {}", report.upstream_tests_pass);
            println!("conformance_pass: {}", report.conformance_pass);
            println!("fullscreen_run_pass: {}", report.fullscreen_run_pass);
            println!("tier: {:?}", report.tier);
            Ok(())
        }
        "build" => {
            let repo_id = args.get(2).ok_or_else(|| "missing repo id".to_string())?;
            run_action("build", &workspace, repo_id, args.iter().any(|arg| arg == "--execute"))
        }
        "test" => {
            let repo_id = args.get(2).ok_or_else(|| "missing repo id".to_string())?;
            run_action("test", &workspace, repo_id, args.iter().any(|arg| arg == "--execute"))
        }
        "run" => {
            let repo_id = args.get(2).ok_or_else(|| "missing repo id".to_string())?;
            run_action("run", &workspace, repo_id, args.iter().any(|arg| arg == "--execute"))
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
                workspace.write_manifest(&spec, &RepoWorkspace::adapter_template(&spec))?;
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
                let status = Command::new("git")
                    .args(["pull", "--ff-only"])
                    .current_dir(&source_root)
                    .status()
                    .map_err(|err| err.to_string())?;
                if !status.success() {
                    return Err(format!("git pull failed with status {status}"));
                }
            } else {
                let status = Command::new("git")
                    .args(["clone", &spec.url, source_root.to_string_lossy().as_ref()])
                    .status()
                    .map_err(|err| err.to_string())?;
                if !status.success() {
                    return Err(format!("git clone failed with status {status}"));
                }
                if let Some(rev) = spec.rev {
                    let status = Command::new("git")
                        .args(["checkout", &rev])
                        .current_dir(&source_root)
                        .status()
                        .map_err(|err| err.to_string())?;
                    if !status.success() {
                        return Err(format!("git checkout failed with status {status}"));
                    }
                }
            }
            println!("synced repo: {repo_id}");
            Ok(())
        }
        _ => Err("usage: macland-cli repo <add|sync> ...".to_string()),
    }
}

fn print_doctor(report: DoctorReport) {
    println!("host.macos={}", report.host.macos);
    println!("host.apple_silicon={}", report.host.apple_silicon);
    for tool in report.tools {
        println!("tool.{}={} ({})", tool.name, tool.found, tool.detail);
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
    let spec = workspace.load_repo_spec(repo_id).unwrap_or_else(|_| RepoSpec::new(repo_id, "", None));
    let source_root = workspace.source_root(&spec);
    let repo_root = if source_root.exists() { source_root } else { workspace.repo_root(&spec) };

    let line = match action {
        "build" => plan.build,
        "test" => plan.test,
        "run" => plan.run,
        _ => Vec::new(),
    };

    println!("repo: {}", manifest.id);
    println!("action: {action}");
    println!("command: {}", line.join(" "));
    println!("cwd: {}", repo_root.display());
    println!(
        "upstream_test_hint: {}",
        CommandPlan::upstream_test_hint(manifest.build_system)
    );
    if execute {
        execute_command_line(&repo_root, &line, &manifest.env)?;
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
    println!("macland-cli commands:");
    println!("  doctor");
    println!("  repo add <git-url> [--rev <commit>]");
    println!("  repo sync <repo-id>");
    println!("  inspect <repo-id>");
    println!("  build <repo-id> [--execute]");
    println!("  test <repo-id> [--execute]");
    println!("  run <repo-id> [--execute]");
}
