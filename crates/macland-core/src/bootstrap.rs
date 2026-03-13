use crate::doctor::DoctorReport;
use crate::workspace_shims::{
    DEPENDENCIES as WORKSPACE_SHIM_DEPENDENCIES, install_workspace_shims,
};
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapPlan {
    pub packages: Vec<&'static str>,
    pub workspace_shims: Vec<&'static str>,
}

impl BootstrapPlan {
    pub fn from_doctor(report: &DoctorReport) -> Self {
        let mut packages = Vec::new();
        let mut workspace_shims = Vec::new();
        for tool in report.missing_tools() {
            let package = match tool {
                "meson" => Some("meson"),
                "ninja" => Some("ninja"),
                "pkg-config" => Some("pkg-config"),
                "Xwayland" => Some("xorg-server"),
                _ => None,
            };
            if let Some(package) = package {
                if !packages.contains(&package) {
                    packages.push(package);
                }
            }
        }
        for dependency in report.missing_native_dependencies() {
            let package = match dependency {
                "xkbcommon" => Some("libxkbcommon"),
                "egl" | "glesv2" => Some("mesa"),
                "epoll-shim" => Some("epoll-shim"),
                "libzip" => Some("libzip"),
                "tomlplusplus" => Some("tomlplusplus"),
                "libmagic" => Some("libmagic"),
                "libheif" => Some("libheif"),
                "pugixml" => Some("pugixml"),
                "xcursor" => Some("libxcursor"),
                "re2" => Some("re2"),
                "muparser" => Some("muparser"),
                "libdrm" | "gbm" | "libinput" | "libevdev" | "libudev" => None,
                _ => None,
            };
            if let Some(package) = package {
                if !packages.contains(&package) {
                    packages.push(package);
                }
            }
            if WORKSPACE_SHIM_DEPENDENCIES.contains(&dependency)
                && !workspace_shims.contains(&dependency)
            {
                workspace_shims.push(dependency);
            }
        }
        Self {
            packages,
            workspace_shims,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.packages.is_empty() && self.workspace_shims.is_empty()
    }
}

pub fn execute_bootstrap(plan: &BootstrapPlan) -> Result<(), String> {
    if !plan.packages.is_empty() {
        if !Path::new("/opt/homebrew/bin/brew").exists()
            && !Path::new("/usr/local/bin/brew").exists()
        {
            return Err(format!(
                "homebrew is required to install missing packages: {}",
                plan.packages.join(", ")
            ));
        }

        let status = Command::new("brew")
            .arg("install")
            .args(&plan.packages)
            .status()
            .map_err(|err| err.to_string())?;

        if !status.success() {
            return Err(format!("brew install failed with status {status}"));
        }
    }

    if let Some(workspace_root) = find_workspace_root() {
        install_workspace_shims(&workspace_root)?;
    }

    Ok(())
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
    use super::BootstrapPlan;
    use crate::backend::BackendCapabilities;
    use crate::doctor::{DoctorReport, HostStatus, NativeDependencyStatus, ToolStatus};

    #[test]
    fn plans_missing_packages() {
        let report = DoctorReport {
            tools: vec![
                ToolStatus {
                    name: "meson",
                    found: false,
                    detail: "missing".to_string(),
                },
                ToolStatus {
                    name: "ninja",
                    found: false,
                    detail: "missing".to_string(),
                },
            ],
            native_dependencies: vec![
                NativeDependencyStatus {
                    name: "xkbcommon",
                    found: false,
                    detail: "missing".to_string(),
                },
                NativeDependencyStatus {
                    name: "egl",
                    found: false,
                    detail: "missing".to_string(),
                },
                NativeDependencyStatus {
                    name: "libudev",
                    found: false,
                    detail: "missing".to_string(),
                },
                NativeDependencyStatus {
                    name: "libevdev",
                    found: false,
                    detail: "missing".to_string(),
                },
            ],
            host: HostStatus {
                apple_silicon: true,
                macos: true,
            },
            backend: BackendCapabilities::macos_defaults(),
        };

        let plan = BootstrapPlan::from_doctor(&report);
        assert_eq!(
            plan.packages,
            vec!["meson", "ninja", "libxkbcommon", "mesa"]
        );
        assert_eq!(plan.workspace_shims, vec!["libudev", "libevdev"]);
    }
}
