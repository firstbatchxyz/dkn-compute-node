use dkn_executor::Model;
use dkn_p2p::libp2p::PeerId;
use dkn_utils::{
    payloads::{SpecModelPerformance, Specs},
    SemanticVersion,
};
use std::collections::HashMap;
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind};

pub struct SpecCollector {
    /// System information object, this is expected to be created only once
    /// as per the [docs](https://github.com/GuillaumeGomez/sysinfo?tab=readme-ov-file#good-practice--performance-tips).
    system: sysinfo::System,
    /// Used models.
    models: Vec<String>,
    /// Model performances
    model_perf: HashMap<String, SpecModelPerformance>,
    /// Version string.
    version: String,
    /// Execution platform, mainly for diagnostics.
    exec_platform: String,
    /// Peer ID of the node, used for identification in the network.
    peer_id: String,
    // GPU adapter infos, showing information about the available GPUs.
    // gpus: Vec<wgpu::AdapterInfo>,
}

impl SpecCollector {
    pub fn new(
        models: Vec<String>,
        model_perf: HashMap<Model, SpecModelPerformance>,
        version: SemanticVersion,
        exec_platform: String,
        peer_id: PeerId,
    ) -> Self {
        log::info!("Creating spec collector with version {version} and platform {exec_platform} and models {models:?}");
        SpecCollector {
            system: sysinfo::System::new_with_specifics(Self::get_refresh_specifics()),
            models,
            model_perf: model_perf
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
            version: version.to_string(),
            exec_platform,
            peer_id: peer_id.to_string(),
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
            version: self.version.clone(),
            model_perf: self.model_perf.clone(),
            exec_platform: Some(self.exec_platform.clone()),
            peer_id: Some(self.peer_id.clone()),
            // gpus: self.gpus.clone(),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_specs_serialization() {
        let mut spec_collector = SpecCollector::new(
            vec![Model::Gemma3_4b.to_string()],
            HashMap::from_iter([
                (Model::Gemma3_4b, SpecModelPerformance::PassedWithTPS(100.0)),
                (Model::Gemma3_27b, SpecModelPerformance::ExecutionFailed),
            ]),
            SemanticVersion {
                major: 4,
                minor: 5,
                patch: 1,
            },
            "testing".to_string(),
            PeerId::random(),
        );
        let specs = spec_collector.collect().await;
        assert!(specs.total_mem > 0);
        assert!(specs.free_mem > 0);
        assert!(specs.num_cpus.is_some());
        assert!(specs.cpu_usage > 0.0);
        assert!(!specs.os.is_empty());
        assert!(!specs.arch.is_empty());
        assert!(specs.lookup.is_some());
        assert!(!specs.models.is_empty());
        assert_eq!(specs.model_perf.len(), 2);
        assert_eq!(specs.version, "4.5.1");
        assert_eq!(specs.exec_platform, Some("testing".to_string()));

        // should be serializable to JSON
        assert!(serde_json::to_string_pretty(&specs).is_ok())
    }
}
