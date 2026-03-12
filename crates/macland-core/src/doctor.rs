use crate::backend::BackendCapabilities;
use crate::backend_ffi::sdk_capabilities;
use std::env;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolStatus {
    pub name: &'static str,
    pub found: bool,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DoctorReport {
    pub tools: Vec<ToolStatus>,
    pub native_dependencies: Vec<NativeDependencyStatus>,
    pub host: HostStatus,
    pub backend: BackendCapabilities,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeDependencyStatus {
    pub name: &'static str,
    pub found: bool,
    pub detail: String,
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
        let native_dependency_names = [
            "wayland-client",
            "wayland-server",
            "wayland-protocols",
            "xkbcommon",
            "pixman-1",
            "egl",
            "glesv2",
            "epoll-shim",
            "libzip",
            "tomlplusplus",
            "libmagic",
            "libheif",
            "pugixml",
            "xcursor",
            "re2",
            "muparser",
        ];
        let native_dependencies = native_dependency_names
            .into_iter()
            .map(|name| probe_pkg_config(name))
            .collect();

        Self {
            tools,
            native_dependencies,
            host: HostStatus {
                apple_silicon: env::consts::ARCH == "aarch64",
                macos: env::consts::OS == "macos",
            },
            backend: sdk_capabilities(),
        }
    }

    pub fn missing_tools(&self) -> Vec<&'static str> {
        self.tools
            .iter()
            .filter(|tool| !tool.found)
            .map(|tool| tool.name)
            .collect()
    }

    pub fn missing_native_dependencies(&self) -> Vec<&'static str> {
        self.native_dependencies
            .iter()
            .filter(|dependency| !dependency.found)
            .map(|dependency| dependency.name)
            .collect()
    }
}

fn locate_tool(name: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH")?;
    env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.exists())
}

fn probe_pkg_config(name: &'static str) -> NativeDependencyStatus {
    let mut command = Command::new("pkg-config");
    command.args(["--modversion", name]);
    if let Some(path) = merged_pkg_config_path() {
        command.env("PKG_CONFIG_PATH", path);
    }

    let detail = match command.output() {
        Ok(output) if output.status.success() => String::from_utf8_lossy(&output.stdout).trim().to_string(),
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            if stderr.is_empty() {
                "missing".to_string()
            } else {
                stderr.lines().next().unwrap_or("missing").to_string()
            }
        }
        Err(_) => "pkg-config unavailable".to_string(),
    };

    NativeDependencyStatus {
        name,
        found: !matches!(detail.as_str(), "missing" | "pkg-config unavailable")
            && !detail.contains("not found"),
        detail,
    }
}

fn merged_pkg_config_path() -> Option<String> {
    let mut paths = Vec::new();
    if let Some(value) = env::var_os("PKG_CONFIG_PATH") {
        paths.extend(env::split_paths(&value).map(|path| path.display().to_string()));
    }

    for candidate in [
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
    ] {
        let resolved = if candidate.starts_with('.') {
            find_workspace_root().map(|root| root.join(candidate))
        } else {
            Some(std::path::PathBuf::from(candidate))
        };
        if let Some(path) = resolved {
            let value = path.display().to_string();
            if path.exists() && !paths.iter().any(|existing| existing == &value) {
                paths.push(value);
            }
        }
    }

    if paths.is_empty() {
        None
    } else {
        Some(paths.join(":"))
    }
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

#[cfg(test)]
mod tests {
    use super::DoctorReport;

    #[test]
    fn gathers_report() {
        let report = DoctorReport::gather();
        assert!(report.tools.iter().any(|tool| tool.name == "swift"));
        assert!(report
            .native_dependencies
            .iter()
            .any(|dependency| dependency.name == "pixman-1"));
        assert!(report.backend.supports_fullscreen_host);
    }
}
