use public_ip_address::response::LookupResponse;
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
    /// Global CPU usage, in percentage.
    cpu_usage: f32,
    /// Operating system name, e.g. `linux`, `macos`, `windows`.
    os: String,
    /// CPU architecture, e.g. `x86_64`, `aarch64`.
    arch: String,
    /// Public IP lookup response.
    lookup: Option<LookupResponse>,
    /// Used models.
    models: Vec<String>,
    // GPU adapter infos, showing information about the available GPUs.
    // gpus: Vec<wgpu::AdapterInfo>,
}

pub struct SpecCollector {
    /// System information object, this is expected to be created only once
    /// as per the [docs](https://github.com/GuillaumeGomez/sysinfo?tab=readme-ov-file#good-practice--performance-tips).
    system: sysinfo::System,
    /// Used models.
    models: Vec<String>,
    // GPU adapter infos, showing information about the available GPUs.
    // gpus: Vec<wgpu::AdapterInfo>,
}

impl Default for SpecCollector {
    fn default() -> Self {
        Self::new(vec![])
    }
}

impl SpecCollector {
    pub fn new(models: Vec<String>) -> Self {
        SpecCollector {
            system: sysinfo::System::new_with_specifics(Self::get_refresh_specifics()),
            models,
            // gpus: wgpu::Instance::default()
            //     .enumerate_adapters(wgpu::Backends::all())
            //     .into_iter()
            //     .map(|a| a.get_info())
            //     .collect(),
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
            lookup: public_ip_address::perform_lookup(None).await.ok(),
            models: self.models.clone(),
            // gpus: self.gpus.clone(),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_print_specs() {
        let mut spec_collector = SpecCollector::new(vec!["gpt-4o".to_string()]);
        let specs = spec_collector.collect().await;
        assert!(specs.total_mem > 0);
        assert!(specs.free_mem > 0);
        assert!(specs.num_cpus.is_some());
        assert!(specs.cpu_usage > 0.0);
        assert!(!specs.os.is_empty());
        assert!(!specs.arch.is_empty());
        assert!(specs.lookup.is_some());
    }
}
