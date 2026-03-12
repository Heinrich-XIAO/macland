#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SupportTier {
    Experimental,
    Tier1,
    Tier2,
    Tier3,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupportReport {
    pub buildable: bool,
    pub upstream_tests_pass: bool,
    pub conformance_pass: bool,
    pub fullscreen_run_pass: bool,
    pub tier: SupportTier,
}

impl SupportReport {
    pub fn inspect_defaults() -> Self {
        Self {
            buildable: false,
            upstream_tests_pass: false,
            conformance_pass: false,
            fullscreen_run_pass: false,
            tier: SupportTier::Experimental,
        }
    }
}

