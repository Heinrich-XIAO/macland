use crate::adapter::AdapterManifest;
use std::fs;
use std::path::{Path, PathBuf};

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
        let rev = fs::read_to_string(root.join(".repo-rev")).ok().map(|value| value.trim().to_string());
        Ok(RepoSpec::new(repo_id, url.trim(), rev))
    }

    pub fn load_manifest(&self, spec: &RepoSpec) -> Result<AdapterManifest, String> {
        let path = self.repo_root(spec).join("macland.toml");
        let contents = fs::read_to_string(&path).map_err(|err| err.to_string())?;
        AdapterManifest::from_toml(&contents)
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

#[cfg(test)]
mod tests {
    use super::{RepoSpec, RepoWorkspace};
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
}
