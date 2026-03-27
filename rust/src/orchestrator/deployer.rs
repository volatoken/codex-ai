use tracing::info;

/// Decides whether a tool should be deployed via Docker or run directly.
pub enum DeployStrategy {
    /// Full Docker container (for complex tools with dependencies)
    Docker,
    /// Direct Python process (for lightweight scripts)
    DirectPython,
}

/// Analyze a project and decide the best deploy strategy.
pub fn decide_strategy(_project_dir: &str, plan: &serde_json::Value) -> DeployStrategy {
    let tool_type = plan["tool_type"].as_str().unwrap_or("general");
    let has_deps = plan["has_external_deps"].as_bool().unwrap_or(true);

    match tool_type {
        "simple-script" | "cron-job" if !has_deps => {
            info!("Deploy strategy: Direct Python (lightweight tool)");
            DeployStrategy::DirectPython
        }
        _ => {
            info!("Deploy strategy: Docker (complex tool)");
            DeployStrategy::Docker
        }
    }
}
