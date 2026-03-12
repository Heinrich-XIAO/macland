use crate::backend::BackendCapabilities;
use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolStatus {
    pub name: &'static str,
    pub found: bool,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorReport {
    pub tools: Vec<ToolStatus>,
    pub host: HostStatus,
    pub backend: BackendCapabilities,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostStatus {
    pub apple_silicon: bool,
    pub macos: bool,
}

impl DoctorReport {
    pub fn gather() -> Self {
        let tool_names = ["swift", "cargo", "rustc", "clang", "meson", "ninja", "pkg-config"];
        let tools = tool_names
            .into_iter()
            .map(|name| match locate_tool(name) {
                Some(path) => ToolStatus {
                    name,
                    found: true,
                    detail: path.display().to_string(),
                },
                None => ToolStatus {
                    name,
                    found: false,
                    detail: "missing".to_string(),
                },
            })
            .collect();

        Self {
            tools,
            host: HostStatus {
                apple_silicon: env::consts::ARCH == "aarch64",
                macos: env::consts::OS == "macos",
            },
            backend: BackendCapabilities::macos_defaults(),
        }
    }

    pub fn missing_tools(&self) -> Vec<&'static str> {
        self.tools
            .iter()
            .filter(|tool| !tool.found)
            .map(|tool| tool.name)
            .collect()
    }
}

fn locate_tool(name: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.exists())
}

#[cfg(test)]
mod tests {
    use super::DoctorReport;

    #[test]
    fn gathers_report() {
        let report = DoctorReport::gather();
        assert!(report.tools.iter().any(|tool| tool.name == "swift"));
        assert!(report.backend.supports_fullscreen_host);
    }
}
