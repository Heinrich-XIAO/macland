use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("macland-{name}-{nanos}"))
}

fn copy_dir_all(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let ty = entry.file_type().unwrap();
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()));
        } else {
            fs::copy(entry.path(), dst.join(entry.file_name())).unwrap();
        }
    }
}

fn run(cmd: &mut Command) {
    let status = cmd.status().unwrap();
    assert!(status.success(), "command failed with status {status}");
}

fn output(cmd: &mut Command) -> String {
    let output = cmd.output().unwrap();
    assert!(
        output.status.success(),
        "command failed with status {}",
        output.status
    );
    String::from_utf8(output.stdout).unwrap()
}

fn write_host_stub(path: &Path) {
    fs::write(
        path,
        r#"#!/bin/sh
set -eu
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
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn create_git_fixture(fixture_root: &Path, source_repo: &Path) {
    copy_dir_all(fixture_root, source_repo);
    run(Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(source_repo));
    run(Command::new("git")
        .args(["add", "."])
        .current_dir(source_repo));
    run(Command::new("git")
        .args([
            "-c",
            "user.name=macland",
            "-c",
            "user.email=macland@example.invalid",
            "commit",
            "-m",
            "fixture",
        ])
        .current_dir(source_repo));
}

fn commit_all(repo: &Path, message: &str) {
    run(Command::new("git").args(["add", "."]).current_dir(repo));
    run(Command::new("git")
        .args([
            "-c",
            "user.name=macland",
            "-c",
            "user.email=macland@example.invalid",
            "commit",
            "-m",
            message,
        ])
        .current_dir(repo));
}

#[test]
fn cli_exercises_repo_workflow() {
    let workspace = unique_temp_dir("workspace");
    let source_repo = unique_temp_dir("source");
    let fixture_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("example-compositor-template");

    create_git_fixture(&fixture_root, &source_repo);

    fs::create_dir_all(&workspace).unwrap();
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_macland-cli"));
    let repo_url = source_repo.display().to_string();

    output(
        Command::new(&binary)
            .args(["repo", "add", &repo_url, "--rev", "main"])
            .current_dir(&workspace),
    );

    let manifest_path = workspace
        .join("repos")
        .join(source_repo.file_name().unwrap())
        .join("macland.toml");
    let manifest = fs::read_to_string(fixture_root.join("macland.toml"))
        .unwrap()
        .replace("REPLACE_REPO_URL", &repo_url);
    fs::write(&manifest_path, manifest).unwrap();

    run(Command::new(&binary)
        .args([
            "repo",
            "sync",
            source_repo.file_name().unwrap().to_str().unwrap(),
        ])
        .current_dir(&workspace));
    run(Command::new(&binary)
        .args([
            "build",
            source_repo.file_name().unwrap().to_str().unwrap(),
            "--execute",
        ])
        .current_dir(&workspace));
    run(Command::new(&binary)
        .args([
            "test",
            source_repo.file_name().unwrap().to_str().unwrap(),
            "--upstream",
            "--execute",
        ])
        .current_dir(&workspace));

    let host_stub = workspace.join("host-stub.sh");
    write_host_stub(&host_stub);
    run(Command::new(&binary)
        .args([
            "test",
            source_repo.file_name().unwrap().to_str().unwrap(),
            "--conformance",
            "--execute",
        ])
        .env("MACLAND_HOST_BINARY", &host_stub)
        .current_dir(&workspace));
    run(Command::new(&binary)
        .args([
            "run",
            source_repo.file_name().unwrap().to_str().unwrap(),
            "--windowed-debug",
            "--execute",
        ])
        .env("MACLAND_HOST_BINARY", &host_stub)
        .current_dir(&workspace));

    let inspect_output = output(
        Command::new(&binary)
            .args([
                "inspect",
                source_repo.file_name().unwrap().to_str().unwrap(),
            ])
            .current_dir(&workspace),
    );
    assert!(inspect_output.contains("conformance_pass: true"));
    assert!(inspect_output.contains("fullscreen_run_pass: true"));

    assert!(
        workspace
            .join("repos")
            .join(source_repo.file_name().unwrap())
            .join("source")
            .join("bin")
            .join("example-compositor")
            .exists()
    );
}

#[test]
fn cli_autodetects_cargo_repo_workflow() {
    let workspace = unique_temp_dir("cargo-workspace");
    let source_repo = unique_temp_dir("cargo-source");
    let fixture_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("cargo-compositor-template");

    create_git_fixture(&fixture_root, &source_repo);
    fs::create_dir_all(&workspace).unwrap();

    let binary = PathBuf::from(env!("CARGO_BIN_EXE_macland-cli"));
    let repo_url = source_repo.display().to_string();
    let repo_id = source_repo.file_name().unwrap().to_str().unwrap();

    run(Command::new(&binary)
        .args(["repo", "add", &repo_url, "--rev", "main"])
        .current_dir(&workspace));
    run(Command::new(&binary)
        .args(["repo", "sync", repo_id])
        .current_dir(&workspace));

    let manifest =
        fs::read_to_string(workspace.join("repos").join(repo_id).join("macland.toml")).unwrap();
    assert!(manifest.contains("build_system = \"cargo\""));
    assert!(
        manifest.contains("entrypoint = [\"cargo\", \"run\", \"--bin\", \"cargo-compositor\"]")
    );

    run(Command::new(&binary)
        .args(["build", repo_id, "--execute"])
        .current_dir(&workspace));
    run(Command::new(&binary)
        .args(["test", repo_id, "--upstream", "--execute"])
        .current_dir(&workspace));
    let inspect_output = output(
        Command::new(&binary)
            .args(["inspect", repo_id])
            .current_dir(&workspace),
    );
    assert!(inspect_output.contains("buildable: true"));
    assert!(inspect_output.contains("upstream_tests_pass: true"));
}

#[test]
fn cli_autodetects_cmake_repo_workflow() {
    let workspace = unique_temp_dir("cmake-workspace");
    let source_repo = unique_temp_dir("cmake-source");
    let fixture_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("cmake-compositor-template");

    create_git_fixture(&fixture_root, &source_repo);
    fs::create_dir_all(&workspace).unwrap();

    let binary = PathBuf::from(env!("CARGO_BIN_EXE_macland-cli"));
    let repo_url = source_repo.display().to_string();
    let repo_id = source_repo.file_name().unwrap().to_str().unwrap();

    run(Command::new(&binary)
        .args(["repo", "add", &repo_url, "--rev", "main"])
        .current_dir(&workspace));
    run(Command::new(&binary)
        .args(["repo", "sync", repo_id])
        .current_dir(&workspace));

    let manifest =
        fs::read_to_string(workspace.join("repos").join(repo_id).join("macland.toml")).unwrap();
    assert!(manifest.contains("build_system = \"cmake\""));

    run(Command::new(&binary)
        .args(["build", repo_id, "--execute"])
        .current_dir(&workspace));
    run(Command::new(&binary)
        .args(["test", repo_id, "--upstream", "--execute"])
        .current_dir(&workspace));

    let inspect_output = output(
        Command::new(&binary)
            .args(["inspect", repo_id])
            .current_dir(&workspace),
    );
    assert!(inspect_output.contains("buildable: true"));
    assert!(inspect_output.contains("upstream_tests_pass: true"));
}

#[test]
fn cli_autodetects_meson_repo_workflow() {
    let workspace = unique_temp_dir("meson-workspace");
    let source_repo = unique_temp_dir("meson-source");
    let fixture_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("meson-compositor-template");

    create_git_fixture(&fixture_root, &source_repo);
    fs::create_dir_all(&workspace).unwrap();

    let binary = PathBuf::from(env!("CARGO_BIN_EXE_macland-cli"));
    let repo_url = source_repo.display().to_string();
    let repo_id = source_repo.file_name().unwrap().to_str().unwrap();

    run(Command::new(&binary)
        .args(["repo", "add", &repo_url, "--rev", "main"])
        .current_dir(&workspace));
    run(Command::new(&binary)
        .args(["repo", "sync", repo_id])
        .current_dir(&workspace));

    let manifest =
        fs::read_to_string(workspace.join("repos").join(repo_id).join("macland.toml")).unwrap();
    assert!(manifest.contains("build_system = \"meson\""));
    assert!(manifest.contains("entrypoint = [\"./build/demo-meson-compositor\"]"));

    run(Command::new(&binary)
        .args(["build", repo_id, "--execute"])
        .current_dir(&workspace));
    run(Command::new(&binary)
        .args(["test", repo_id, "--upstream", "--execute"])
        .current_dir(&workspace));

    let host_stub = workspace.join("host-stub.sh");
    write_host_stub(&host_stub);
    run(Command::new(&binary)
        .args(["run", repo_id, "--windowed-debug", "--execute"])
        .env("MACLAND_HOST_BINARY", &host_stub)
        .current_dir(&workspace));

    let inspect_output = output(
        Command::new(&binary)
            .args(["inspect", repo_id])
            .env("MACLAND_HOST_BINARY", &host_stub)
            .current_dir(&workspace),
    );
    assert!(inspect_output.contains("buildable: true"));
    assert!(inspect_output.contains("upstream_tests_pass: true"));
}

#[test]
fn repo_sync_initializes_recursive_submodules() {
    let workspace = unique_temp_dir("submodule-workspace");
    let parent_repo = unique_temp_dir("submodule-parent");
    let child_repo = unique_temp_dir("submodule-child");

    fs::create_dir_all(&workspace).unwrap();
    fs::create_dir_all(&parent_repo).unwrap();
    fs::create_dir_all(&child_repo).unwrap();

    fs::write(child_repo.join("README.md"), "child\n").unwrap();
    run(Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(&child_repo));
    commit_all(&child_repo, "child");

    fs::write(parent_repo.join("README.md"), "parent\n").unwrap();
    run(Command::new("git")
        .args(["init", "-b", "main"])
        .current_dir(&parent_repo));
    run(Command::new("git")
        .args([
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            child_repo.to_str().unwrap(),
            "vendor/child",
        ])
        .current_dir(&parent_repo));
    commit_all(&parent_repo, "parent");

    let binary = PathBuf::from(env!("CARGO_BIN_EXE_macland-cli"));
    let repo_url = parent_repo.display().to_string();
    let repo_id = parent_repo.file_name().unwrap().to_str().unwrap();

    run(Command::new(&binary)
        .args(["repo", "add", &repo_url, "--rev", "main"])
        .current_dir(&workspace));
    run(Command::new(&binary)
        .args(["repo", "sync", repo_id])
        .current_dir(&workspace));

    assert!(
        workspace
            .join("repos")
            .join(repo_id)
            .join("source")
            .join("vendor")
            .join("child")
            .join("README.md")
            .exists()
    );
}
