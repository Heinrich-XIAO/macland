use crate::adapter::{AdapterManifest, BuildSystem};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoSpec {
    pub id: String,
    pub url: String,
    pub rev: Option<String>,
}

impl RepoSpec {
    pub fn new(id: impl Into<String>, url: impl Into<String>, rev: Option<String>) -> Self {
        Self {
            id: id.into(),
            url: url.into(),
            rev,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoWorkspace {
    root: PathBuf,
}

impl RepoWorkspace {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn repo_root(&self, spec: &RepoSpec) -> PathBuf {
        self.root.join("repos").join(&spec.id)
    }

    pub fn source_root(&self, spec: &RepoSpec) -> PathBuf {
        self.repo_root(spec).join("source")
    }

    pub fn artifacts_root(&self, spec: &RepoSpec) -> PathBuf {
        self.repo_root(spec).join("artifacts")
    }

    pub fn override_root(&self, spec: &RepoSpec) -> PathBuf {
        self.root.join("overrides").join(&spec.id)
    }

    pub fn override_manifest_path(&self, spec: &RepoSpec) -> PathBuf {
        self.override_root(spec).join("macland.toml")
    }

    pub fn override_patches_root(&self, spec: &RepoSpec) -> PathBuf {
        self.override_root(spec).join("patches")
    }

    pub fn write_manifest(&self, spec: &RepoSpec, contents: &str) -> Result<PathBuf, String> {
        let root = self.repo_root(spec);
        fs::create_dir_all(&root).map_err(|err| err.to_string())?;
        let path = root.join("macland.toml");
        fs::write(&path, contents).map_err(|err| err.to_string())?;
        Ok(path)
    }

    pub fn write_repo_spec(&self, spec: &RepoSpec) -> Result<(), String> {
        let root = self.repo_root(spec);
        fs::create_dir_all(&root).map_err(|err| err.to_string())?;
        fs::write(root.join(".repo-url"), &spec.url).map_err(|err| err.to_string())?;
        if let Some(rev) = &spec.rev {
            fs::write(root.join(".repo-rev"), rev).map_err(|err| err.to_string())?;
        }
        Ok(())
    }

    pub fn load_repo_spec(&self, repo_id: &str) -> Result<RepoSpec, String> {
        let probe = RepoSpec::new(repo_id, "", None);
        let root = self.repo_root(&probe);
        let url = fs::read_to_string(root.join(".repo-url")).map_err(|err| err.to_string())?;
        let rev = fs::read_to_string(root.join(".repo-rev"))
            .ok()
            .map(|value| value.trim().to_string());
        Ok(RepoSpec::new(repo_id, url.trim(), rev))
    }

    pub fn load_manifest(&self, spec: &RepoSpec) -> Result<AdapterManifest, String> {
        let repo_manifest_path = self.repo_root(spec).join("macland.toml");
        let override_manifest_path = self.override_manifest_path(spec);

        if let Ok(contents) = fs::read_to_string(&repo_manifest_path) {
            let manifest = AdapterManifest::from_toml(&contents)?;
            if !is_uninitialized_manifest(&manifest) || !override_manifest_path.exists() {
                return Ok(manifest);
            }
        }

        let contents =
            fs::read_to_string(&override_manifest_path).map_err(|err| err.to_string())?;
        AdapterManifest::from_toml(&contents)
    }

    pub fn seed_manifest_from_override(
        &self,
        spec: &RepoSpec,
        overwrite_uninitialized: bool,
    ) -> Result<Option<PathBuf>, String> {
        let override_manifest_path = self.override_manifest_path(spec);
        if !override_manifest_path.exists() {
            return Ok(None);
        }

        let override_contents =
            fs::read_to_string(&override_manifest_path).map_err(|err| err.to_string())?;
        let repo_root = self.repo_root(spec);
        fs::create_dir_all(&repo_root).map_err(|err| err.to_string())?;
        let repo_manifest_path = repo_root.join("macland.toml");

        let should_write = match fs::read_to_string(&repo_manifest_path) {
            Ok(current) => {
                if !overwrite_uninitialized {
                    false
                } else {
                    AdapterManifest::from_toml(&current)
                        .map(|manifest| is_uninitialized_manifest(&manifest))
                        .unwrap_or(false)
                }
            }
            Err(_) => true,
        };

        if should_write {
            fs::write(&repo_manifest_path, override_contents).map_err(|err| err.to_string())?;
            Ok(Some(repo_manifest_path))
        } else {
            Ok(None)
        }
    }

    pub fn apply_override_patches(&self, spec: &RepoSpec) -> Result<Vec<PathBuf>, String> {
        let patches_root = self.override_patches_root(spec);
        if !patches_root.is_dir() {
            return Ok(Vec::new());
        }

        let source_root = self.source_root(spec);
        let mut patch_paths: Vec<PathBuf> = fs::read_dir(&patches_root)
            .map_err(|err| err.to_string())?
            .filter_map(|entry| entry.ok().map(|value| value.path()))
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("patch"))
            .collect();
        patch_paths.sort();

        let mut applied = Vec::new();
        for patch_path in patch_paths {
            if git_command_succeeds(
                &source_root,
                ["apply", "--reverse", "--check", patch_path.to_string_lossy().as_ref()],
            )? {
                continue;
            }

            if !git_command_succeeds(
                &source_root,
                ["apply", "--check", patch_path.to_string_lossy().as_ref()],
            )? {
                return Err(format!(
                    "failed to apply override patch {}",
                    patch_path.display()
                ));
            }

            run_git(
                &source_root,
                ["apply", patch_path.to_string_lossy().as_ref()],
            )?;
            applied.push(patch_path);
        }

        Ok(applied)
    }

    pub fn adapter_template(spec: &RepoSpec) -> String {
        format!(
            r#"id = "{id}"
repo = "{repo}"
rev = "{rev}"
build_system = "custom"
configure = []
build = []
test = []
entrypoint = []
patch_policy = "prefer-none"
sdk_features = []
protocol_expectations = []

[env]
"#,
            id = spec.id,
            repo = spec.url,
            rev = spec.rev.clone().unwrap_or_else(|| "main".to_string()),
        )
    }

    pub fn ensure_root(&self) -> Result<(), String> {
        fs::create_dir_all(&self.root).map_err(|err| err.to_string())
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

fn is_uninitialized_manifest(manifest: &AdapterManifest) -> bool {
    manifest.build_system == BuildSystem::Custom
        && manifest.configure.is_empty()
        && manifest.build.is_empty()
        && manifest.test.is_empty()
        && manifest.entrypoint.is_empty()
}

fn git_command_succeeds<const N: usize>(cwd: &Path, args: [&str; N]) -> Result<bool, String> {
    let status = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .status()
        .map_err(|err| err.to_string())?;
    Ok(status.success())
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
        Err(format!("git {} failed with status {status}", args.join(" ")))
    }
}

#[cfg(test)]
mod tests {
    use super::{RepoSpec, RepoWorkspace};
    use crate::adapter::BuildSystem;
    use std::fs;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn writes_and_reads_manifest() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("macland-tests-{suffix}"));
        let workspace = RepoWorkspace::new(&root);
        workspace.ensure_root().unwrap();
        let spec = RepoSpec::new("labwc", "https://example.com/labwc.git", None);
        workspace
            .write_manifest(
                &spec,
                r#"
                id = "labwc"
                repo = "https://example.com/labwc.git"
                rev = "main"
                build_system = "meson"
                configure = ["meson", "setup", "build"]
                build = ["meson", "compile", "-C", "build"]
                test = ["meson", "test", "-C", "build"]
                entrypoint = ["./build/labwc"]
                sdk_features = ["metal-fast-path"]
                protocol_expectations = ["xdg-shell"]
                patch_policy = "prefer-none"
                "#,
            )
            .unwrap();

        let manifest = workspace.load_manifest(&spec).unwrap();
        assert_eq!(manifest.id, "labwc");
    }

    #[test]
    fn template_includes_repo_identity() {
        let spec = RepoSpec::new(
            "weston",
            "https://example.com/weston.git",
            Some("14.0".to_string()),
        );
        let template = RepoWorkspace::adapter_template(&spec);
        assert!(template.contains("id = \"weston\""));
        assert!(template.contains("rev = \"14.0\""));
    }

    #[test]
    fn repo_spec_round_trips() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("macland-tests-{suffix}"));
        let workspace = RepoWorkspace::new(&root);
        let spec = RepoSpec::new(
            "cagebreak",
            "https://example.com/cagebreak.git",
            Some("main".to_string()),
        );
        workspace.write_repo_spec(&spec).unwrap();
        let loaded = workspace.load_repo_spec("cagebreak").unwrap();
        assert_eq!(loaded, spec);
    }

    #[test]
    fn loads_override_manifest_when_repo_manifest_is_uninitialized() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("macland-tests-{suffix}"));
        let workspace = RepoWorkspace::new(&root);
        workspace.ensure_root().unwrap();
        let spec = RepoSpec::new("weston", "https://example.com/weston.git", None);
        workspace
            .write_manifest(&spec, &RepoWorkspace::adapter_template(&spec))
            .unwrap();
        let override_root = workspace.override_root(&spec);
        fs::create_dir_all(&override_root).unwrap();
        fs::write(
            override_root.join("macland.toml"),
            r#"
                id = "weston"
                repo = "https://example.com/weston.git"
                rev = "main"
                build_system = "meson"
                configure = ["meson", "setup", "build"]
                build = ["meson", "compile", "-C", "build"]
                test = ["meson", "test", "-C", "build"]
                entrypoint = ["./build/frontend/weston"]
                sdk_features = ["metal-fast-path"]
                protocol_expectations = ["xdg-shell"]
                patch_policy = "prefer-none"
                "#,
        )
        .unwrap();

        let manifest = workspace.load_manifest(&spec).unwrap();
        assert_eq!(manifest.build_system, BuildSystem::Meson);
        assert_eq!(manifest.entrypoint, vec!["./build/frontend/weston".to_string()]);
    }

    #[test]
    fn applies_override_patches_once() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("macland-tests-{suffix}"));
        let workspace = RepoWorkspace::new(&root);
        workspace.ensure_root().unwrap();
        let spec = RepoSpec::new("patchy", "https://example.com/patchy.git", None);
        let source_root = workspace.source_root(&spec);
        fs::create_dir_all(&source_root).unwrap();

        Command::new("git")
            .args(["init"])
            .current_dir(&source_root)
            .status()
            .unwrap();
        fs::write(source_root.join("demo.txt"), "before\n").unwrap();
        Command::new("git")
            .args(["add", "demo.txt"])
            .current_dir(&source_root)
            .status()
            .unwrap();
        Command::new("git")
            .args([
                "-c",
                "user.email=macland@example.com",
                "-c",
                "user.name=Macland",
                "commit",
                "-m",
                "init",
            ])
            .current_dir(&source_root)
            .status()
            .unwrap();

        let patches_root = workspace.override_patches_root(&spec);
        fs::create_dir_all(&patches_root).unwrap();
        fs::write(
            patches_root.join("0001-demo.patch"),
            r#"diff --git a/demo.txt b/demo.txt
index df967b9..3a6eb07 100644
--- a/demo.txt
+++ b/demo.txt
@@ -1 +1 @@
-before
+after
"#,
        )
        .unwrap();

        let applied = workspace.apply_override_patches(&spec).unwrap();
        assert_eq!(applied.len(), 1);
        assert_eq!(fs::read_to_string(source_root.join("demo.txt")).unwrap(), "after\n");

        let applied_again = workspace.apply_override_patches(&spec).unwrap();
        assert!(applied_again.is_empty());
    }
}
