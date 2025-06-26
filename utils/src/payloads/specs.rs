use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Topic used within [`crate::DriaMessage`] for specs messages.
pub const SPECS_TOPIC: &str = "specs";

#[derive(Serialize, Deserialize)]
pub struct SpecsRequest {
    /// UUID of the specs request, prevents replays.
    pub specs_id: Uuid,
    /// Node specs.
    pub specs: Specs,
    /// Address of the node, used by frontend etc. instead of peer id.
    pub address: String,
}

#[derive(Serialize, Deserialize)]
pub struct SpecsResponse {
    /// UUID of the specs request, prevents replays.
    pub specs_id: Uuid,
}

/// The specs of a node, containing information about the hardware and software it runs on.
///
/// Optional values are done so for backwards compatibility, as some fields were added later.
#[derive(Debug, Serialize, Deserialize)]
pub struct Specs {
    /// Total memory in bytes
    pub total_mem: u64,
    /// Free memory in bytes
    pub free_mem: u64,
    /// Number of physical CPU cores.
    pub num_cpus: Option<usize>,
    /// Global CPU usage, in percentage.
    pub cpu_usage: f32,
    /// Operating system name, e.g. `linux`, `macos`, `windows`.
    pub os: String,
    /// CPU architecture, e.g. `x86_64`, `aarch64`.
    pub arch: String,
    /// Public IP lookup response.
    pub lookup: Option<public_ip_address::response::LookupResponse>,
    /// Models server by this node.
    pub models: Vec<String>,
    /// Model performance metrics, keyed by model name.
    pub model_perf: HashMap<String, SpecModelPerformance>,
    /// Node version, e.g. `0.1.0`.
    pub version: String,
    /// Name of the execution platform, e.g. Docker file or Launcher.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exec_platform: Option<String>,
    /// Peer id of the node.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer_id: Option<String>,
    // GPU adapter infos, showing information about the available GPUs.
    // gpus: Vec<wgpu::AdapterInfo>,
}

/// Performance metrics for a model, used in the specs.
///
/// These are measured at the start of the compute node, and those that are not succesfull.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SpecModelPerformance {
    /// Evaluation tokens per second (TPS) for the model that has passed evaluation.
    PassedWithTPS(f64),
    /// Evaluation tokens per second (TPS) for the model that has failed evaluation.
    FailedWithTPS(f64),
    /// Model has timed-out during performance evaluation.
    ///
    /// This can happen if the model is slow to respond or the request takes too long.
    Timeout,
    /// Model is not found for performance evaluation.
    ///
    /// Possible reasons are API key not set, or model not available in the account.
    NotFound,
    /// Model has failed to execute during performance evaluation.
    ///
    /// This can happen if the model is not available, or the request fails for some reason.
    /// One example is OpenRouter, where sometimes models are not available even if they are listed.
    ExecutionFailed,
    /// Model has passed execution performance evaluation, however TPS was not available.
    Passed,
}

impl std::fmt::Display for SpecModelPerformance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpecModelPerformance::PassedWithTPS(tps) => write!(f, "Passed with TPS: {tps:.3}"),
            SpecModelPerformance::FailedWithTPS(tps) => {
                write!(f, "Failed with TPS: {tps:.3}")
            }
            SpecModelPerformance::Timeout => write!(f, "Timeout"),
            SpecModelPerformance::NotFound => write!(f, "Not Found"),
            SpecModelPerformance::ExecutionFailed => write!(f, "Execution Failed"),
            SpecModelPerformance::Passed => write!(f, "Passed"),
        }
    }
}
