use public_ip_address::response::LookupResponse;
use serde::{Deserialize, Serialize};
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind};
use wgpu::AdapterInfo;

/// Machine info & location.
#[derive(Debug, Serialize, Deserialize)]
pub struct Specs {
    /// Total memory in bytes
    total_mem: u64,
    /// Free memory in bytes
    free_mem: u64,
    /// Number of physical CPU cores.
    num_cpus: Option<usize>,
    /// Global CPU usage, in percentage.
    cpu_usage: f32,
    /// Operating system name, e.g. `linux`, `macos`, `windows`.
    os: String,
    /// CPU architecture, e.g. `x86_64`, `aarch64`.
    arch: String,
    /// GPU adapter infos, showing information about the available GPUs.
    gpus: Vec<AdapterInfo>,
    /// Public IP lookup response.
    lookup: Option<LookupResponse>,
}

pub struct SpecCollector {
    /// System information object, this is expected to be created only once
    /// as per the [docs](https://github.com/GuillaumeGomez/sysinfo?tab=readme-ov-file#good-practice--performance-tips).
    system: sysinfo::System,
    /// GPU adapter infos, showing information about the available GPUs.
    gpus: Vec<AdapterInfo>,
}

impl SpecCollector {
    pub fn new() -> Self {
        SpecCollector {
            system: sysinfo::System::new_with_specifics(Self::get_refresh_specifics()),
            gpus: wgpu::Instance::default()
                .enumerate_adapters(wgpu::Backends::all())
                .into_iter()
                .map(|a| a.get_info())
                .collect(),
        }
    }

    /// Returns the selected refresh kinds. It is important to ignore
    /// process values here because it will consume a lot of file-descriptors.
    #[inline(always)]
    fn get_refresh_specifics() -> RefreshKind {
        RefreshKind::nothing()
            .with_cpu(CpuRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything())
    }

    pub async fn collect(&mut self) -> Specs {
        self.system.refresh_specifics(Self::get_refresh_specifics());

        Specs {
            total_mem: self.system.total_memory(),
            free_mem: self.system.free_memory(),
            num_cpus: self.system.physical_core_count(),
            cpu_usage: self.system.global_cpu_usage(),
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            gpus: self.gpus.clone(),
            lookup: public_ip_address::perform_lookup(None).await.ok(),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "run manually"]
    async fn test_print_specs() {
        let mut spec_collector = SpecCollector::new();
        let specs = spec_collector.collect().await;
        println!("{}", serde_json::to_string_pretty(&specs).unwrap());
    }
}
