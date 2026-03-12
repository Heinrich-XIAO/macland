use crate::doctor::DoctorReport;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapPlan {
    pub packages: Vec<&'static str>,
}

impl BootstrapPlan {
    pub fn from_doctor(report: &DoctorReport) -> Self {
        let mut packages = Vec::new();
        for tool in report.missing_tools() {
            let package = match tool {
                "meson" => Some("meson"),
                "ninja" => Some("ninja"),
                "pkg-config" => Some("pkg-config"),
                _ => None,
            };
            if let Some(package) = package {
                if !packages.contains(&package) {
                    packages.push(package);
                }
            }
        }
        Self { packages }
    }

    pub fn is_empty(&self) -> bool {
        self.packages.is_empty()
    }
}

pub fn execute_bootstrap(plan: &BootstrapPlan) -> Result<(), String> {
    if plan.is_empty() {
        return Ok(());
    }

    if !Path::new("/opt/homebrew/bin/brew").exists() && !Path::new("/usr/local/bin/brew").exists() {
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

    if status.success() {
        Ok(())
    } else {
        Err(format!("brew install failed with status {status}"))
    }
}

#[cfg(test)]
mod tests {
    use super::BootstrapPlan;
    use crate::backend::BackendCapabilities;
    use crate::doctor::{DoctorReport, HostStatus, ToolStatus};

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
            host: HostStatus {
                apple_silicon: true,
                macos: true,
            },
            backend: BackendCapabilities::macos_defaults(),
        };

        let plan = BootstrapPlan::from_doctor(&report);
        assert_eq!(plan.packages, vec!["meson", "ninja"]);
    }
}
