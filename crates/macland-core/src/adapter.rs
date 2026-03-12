use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildSystem {
    Meson,
    CMake,
    Cargo,
    Autotools,
    Make,
    Custom,
}

impl BuildSystem {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "meson" => Some(Self::Meson),
            "cmake" => Some(Self::CMake),
            "cargo" => Some(Self::Cargo),
            "autotools" => Some(Self::Autotools),
            "make" => Some(Self::Make),
            "custom" => Some(Self::Custom),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterManifest {
    pub id: String,
    pub repo: String,
    pub rev: String,
    pub build_system: BuildSystem,
    pub configure: Vec<String>,
    pub build: Vec<String>,
    pub test: Vec<String>,
    pub entrypoint: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub sdk_features: Vec<String>,
    pub protocol_expectations: Vec<String>,
    pub patch_policy: String,
}

impl AdapterManifest {
    pub fn from_toml(input: &str) -> Result<Self, String> {
        let mut scalars = BTreeMap::new();
        let mut arrays: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut env = BTreeMap::new();
        let mut section = String::new();

        for raw_line in input.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                section = line.trim_matches(|c| c == '[' || c == ']').to_string();
                continue;
            }
            let (key, value) = line
                .split_once('=')
                .ok_or_else(|| format!("invalid line: {line}"))?;
            let key = key.trim().to_string();
            let value = value.trim();

            if section == "env" {
                env.insert(key, unquote(value));
                continue;
            }

            if value.starts_with('[') && value.ends_with(']') {
                arrays.insert(key, parse_array(value)?);
            } else {
                scalars.insert(key, unquote(value));
            }
        }

        let build_system_value = scalars
            .remove("build_system")
            .ok_or_else(|| "missing build_system".to_string())?;
        let build_system = BuildSystem::parse(&build_system_value)
            .ok_or_else(|| format!("unsupported build_system: {build_system_value}"))?;

        Ok(Self {
            id: take_scalar(&mut scalars, "id")?,
            repo: take_scalar(&mut scalars, "repo")?,
            rev: take_scalar(&mut scalars, "rev")?,
            build_system,
            configure: take_array(&mut arrays, "configure")?,
            build: take_array(&mut arrays, "build")?,
            test: take_array(&mut arrays, "test")?,
            entrypoint: take_array(&mut arrays, "entrypoint")?,
            env,
            sdk_features: take_array(&mut arrays, "sdk_features")?,
            protocol_expectations: take_array(&mut arrays, "protocol_expectations")?,
            patch_policy: take_scalar(&mut scalars, "patch_policy")?,
        })
    }
}

fn take_scalar(map: &mut BTreeMap<String, String>, key: &str) -> Result<String, String> {
    map.remove(key)
        .ok_or_else(|| format!("missing {key}"))
}

fn take_array(map: &mut BTreeMap<String, Vec<String>>, key: &str) -> Result<Vec<String>, String> {
    map.remove(key)
        .ok_or_else(|| format!("missing {key}"))
}

fn unquote(value: &str) -> String {
    value.trim_matches('"').to_string()
}

fn parse_array(value: &str) -> Result<Vec<String>, String> {
    let inner = value.trim_matches(|c| c == '[' || c == ']');
    if inner.trim().is_empty() {
        return Ok(Vec::new());
    }
    Ok(inner.split(',').map(|item| unquote(item.trim())).collect())
}

#[cfg(test)]
mod tests {
    use super::{AdapterManifest, BuildSystem};

    #[test]
    fn parses_manifest() {
        let manifest = AdapterManifest::from_toml(
            r#"
            id = "labwc"
            repo = "https://github.com/labwc/labwc.git"
            rev = "main"
            build_system = "meson"
            configure = ["meson", "setup", "build"]
            build = ["meson", "compile", "-C", "build"]
            test = ["meson", "test", "-C", "build"]
            entrypoint = ["./build/labwc"]
            sdk_features = ["metal-fast-path"]
            protocol_expectations = ["xdg-shell"]
            patch_policy = "prefer-none"

            [env]
            MACLAND_MODE = "1"
            "#,
        )
        .unwrap();

        assert_eq!(manifest.id, "labwc");
        assert_eq!(manifest.build_system, BuildSystem::Meson);
        assert_eq!(manifest.env.get("MACLAND_MODE"), Some(&"1".to_string()));
    }
}

