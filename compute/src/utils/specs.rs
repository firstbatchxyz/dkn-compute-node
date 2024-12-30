use serde::{Deserialize, Serialize};
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind};

/// Machine info & location.
#[derive(Debug, Serialize, Deserialize)]
pub struct Specs {
    /// Total memory in bytes
    total_mem: u64,
    /// Free memory in bytes
    free_mem: u64,
    /// Number of physical CPU cores.
    num_cpus: Option<usize>,
    cpu_usage: f32,
    os: String,
    arch: String,
    family: String,
    gpus: Vec<wgpu::AdapterInfo>,
    lookup: Option<public_ip_address::response::LookupResponse>,
}

pub struct SpecCollector {
    /// System information object, this is expected to be created only once
    /// as per the [docs](https://github.com/GuillaumeGomez/sysinfo?tab=readme-ov-file#good-practice--performance-tips).
    system: sysinfo::System,
    /// GPU adapter infos, showing information about the available GPUs.
    gpus: Vec<wgpu::AdapterInfo>,
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
            family: std::env::consts::FAMILY.to_string(),
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
