use std::io::{self, Write};
use std::ops::ControlFlow;
use std::path::PathBuf;

use dialoguer::{Select, theme::ColorfulTheme};
use dkn_protocol::ModelType;

use crate::error::NodeError;
use crate::inference::{GenerateParams, InferenceEngine};
use crate::models::{ModelCache, ModelDownloader, default_registry, resolve_model};

/// Model metadata for the setup display.
struct SetupModel {
    name: String,
    model_type: ModelType,
    quant: String,
    size_gb: f64,
    ram_needed_gb: f64,
}

/// Hardcoded size estimates (Q4_K_M / Q4_0 defaults) for each registry model.
fn model_size_gb(name: &str) -> Option<(f64, f64)> {
    // (gguf_size_gb, ram_needed_gb)
    match name {
        "lfm2.5:1.2b" => Some((0.8, 1.0)),
        "nanbeige:3b" => Some((2.0, 2.5)),
        "locooperator:4b" => Some((2.5, 3.0)),
        "lfm2.5-vl:1.6b" => Some((1.2, 1.5)),
        "lfm2.5-audio:1.5b" => Some((1.0, 1.5)),
        "qwen3.5:9b" => Some((6.0, 7.0)),
        "qwen3.5:27b" => Some((16.0, 18.0)),
        "qwen3.5:35b-a3b" => Some((20.0, 22.0)),
        "lfm2:24b-a2b" => Some((14.0, 16.0)),
        _ => None,
    }
}

/// Extract the quantization string from a GGUF filename (e.g. "Q4_K_M" from "Foo-Q4_K_M.gguf").
fn extract_quant(hf_file: &str) -> String {
    let stem = hf_file.strip_suffix(".gguf").unwrap_or(hf_file);
    match stem.rfind('-') {
        Some(pos) => stem[pos + 1..].to_string(),
        None => stem.to_string(),
    }
}

/// Detect total system RAM in bytes.
fn detect_ram_bytes() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        if let Ok(contents) = std::fs::read_to_string("/proc/meminfo") {
            for line in contents.lines() {
                if let Some(rest) = line.strip_prefix("MemTotal:") {
                    let rest = rest.trim();
                    if let Some(kb_str) = rest.strip_suffix("kB").or_else(|| rest.strip_suffix("KB"))
                    {
                        if let Ok(kb) = kb_str.trim().parse::<u64>() {
                            return Some(kb * 1024);
                        }
                    }
                }
            }
        }
        None
    }

    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("sysctl")
            .args(["-n", "hw.memsize"])
            .output()
            .ok()?;
        let s = String::from_utf8_lossy(&output.stdout);
        s.trim().parse::<u64>().ok()
    }

    #[cfg(target_os = "windows")]
    {
        let output = std::process::Command::new("wmic")
            .args(["OS", "get", "TotalVisibleMemorySize"])
            .output()
            .ok()?;
        let s = String::from_utf8_lossy(&output.stdout);
        for line in s.lines() {
            if let Ok(kb) = line.trim().parse::<u64>() {
                return Some(kb * 1024);
            }
        }
        None
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        None
    }
}

fn model_type_label(mt: ModelType) -> &'static str {
    match mt {
        ModelType::Text => "Text",
        ModelType::Vision => "Vision",
        ModelType::Audio => "Audio",
    }
}

pub async fn run_setup(data_dir: Option<PathBuf>, gpu_layers: i32) -> Result<(), NodeError> {
    println!();
    println!("  Welcome to Dria Node setup!");
    println!();

    // Detect RAM
    let ram_gb = detect_ram_bytes().map(|b| b as f64 / (1024.0 * 1024.0 * 1024.0));

    if let Some(gb) = ram_gb {
        println!("  System: {:.0} GB RAM detected", gb);
    } else {
        println!("  System: could not detect RAM, showing all models");
    }
    println!();

    // Build model list from registry with size info
    let registry = default_registry();
    let mut models: Vec<SetupModel> = Vec::new();

    for spec in registry.values() {
        if let Some((size_gb, ram_needed_gb)) = model_size_gb(&spec.name) {
            models.push(SetupModel {
                name: spec.name.clone(),
                model_type: spec.model_type,
                quant: extract_quant(&spec.hf_file),
                size_gb,
                ram_needed_gb,
            });
        }
    }

    // Sort by size ascending
    models.sort_by(|a, b| a.size_gb.partial_cmp(&b.size_gb).unwrap());

    // Split into fits / too-large
    let (fits, too_large): (Vec<_>, Vec<_>) = match ram_gb {
        Some(gb) => models
            .into_iter()
            .partition(|m| m.ram_needed_gb < gb),
        None => (models, vec![]),
    };

    if fits.is_empty() {
        println!("  No models fit your available RAM. Minimum recommended: 2 GB.");
        return Ok(());
    }

    // Print too-large models as info
    if !too_large.is_empty() {
        println!("  Models too large for your system:");
        for m in &too_large {
            println!(
                "    - {:<22} (~{:.0} GB) — needs ~{:.0} GB RAM",
                m.name, m.size_gb, m.ram_needed_gb,
            );
        }
        println!();
    }

    // Build display items for model selection
    let model_items: Vec<String> = fits
        .iter()
        .map(|m| {
            format!(
                "{:<22} {:<8} {:<10} ~{:.1} GB",
                m.name,
                model_type_label(m.model_type),
                m.quant,
                m.size_gb,
            )
        })
        .collect();

    let theme = ColorfulTheme::default();

    // Set up cache dir once
    let data_dir = match data_dir {
        Some(d) => d,
        None => dirs::home_dir()
            .ok_or_else(|| NodeError::Config("could not determine home directory".into()))?
            .join(".dria"),
    };
    let models_dir = data_dir.join("models");
    std::fs::create_dir_all(&models_dir)?;
    let cache = ModelCache::new(models_dir)?;

    // Selection + download loop — retries on failure
    let (spec, model_name_final, quant_override) = loop {
        let selection = Select::with_theme(&theme)
            .with_prompt("  Select a model")
            .items(&model_items)
            .default(0)
            .interact()
            .map_err(|e| NodeError::Config(format!("selection error: {e}")))?;

        let chosen = &fits[selection];
        let model_name = &chosen.name;

        // Quantization selection (4-bit vs 8-bit)
        let q8_size = chosen.size_gb * 2.0;
        let q8_ram = chosen.ram_needed_gb * 2.0;
        let q8_fits = ram_gb.map_or(true, |gb| q8_ram < gb);

        let quant_override = if q8_fits {
            let quant_items = vec![
                format!(
                    "4-bit  ({})  ~{:.1} GB — smaller, faster",
                    chosen.quant, chosen.size_gb
                ),
                format!(
                    "8-bit  (Q8_0){}  ~{:.1} GB — better quality",
                    " ".repeat(chosen.quant.len().saturating_sub(4)),
                    q8_size
                ),
                "Back".to_string(),
            ];

            let quant_selection = Select::with_theme(&theme)
                .with_prompt("  Select quantization")
                .items(&quant_items)
                .default(0)
                .interact()
                .map_err(|e| NodeError::Config(format!("selection error: {e}")))?;

            if quant_selection == 2 {
                println!();
                continue;
            } else if quant_selection == 1 {
                Some("Q8_0")
            } else {
                None
            }
        } else {
            println!();
            println!(
                "  Using {} (8-bit needs ~{:.0} GB RAM, you have ~{:.0} GB)",
                chosen.quant,
                q8_ram,
                ram_gb.unwrap_or(0.0)
            );
            None
        };

        println!();

        let spec = match resolve_model(model_name, &registry, quant_override) {
            Some(s) => s,
            None => {
                println!("  Unknown model: {model_name}. Try again.");
                println!();
                continue;
            }
        };

        // Download model
        println!("  Downloading {}...", model_name);
        let model_path = if let Some(path) = cache.get_local_path(&spec) {
            println!("  (already cached)");
            Ok(path)
        } else {
            match ModelDownloader::download(&spec).await {
                Ok(hf_path) => {
                    if let Some(ref expected_sha) = spec.sha256 {
                        if !ModelCache::verify_sha256(&hf_path, expected_sha)? {
                            println!("  SHA-256 mismatch! Try a different model.");
                            println!();
                            continue;
                        }
                    }
                    cache.link_model(&spec, &hf_path).map_err(|e| e.into())
                }
                Err(e) => Err(e),
            }
        };

        let model_path = match model_path {
            Ok(p) => p,
            Err(e) => {
                println!("  Download failed: {e}");
                println!("  Try a different model or quantization.");
                println!();
                continue;
            }
        };

        // Download mmproj if needed
        let mmproj_result = if spec.hf_mmproj_file.is_some() {
            if let Some(path) = cache.get_mmproj_path(&spec) {
                Ok(Some(path))
            } else {
                match ModelDownloader::download_mmproj(&spec).await {
                    Ok(hf_path) => cache.link_mmproj(&spec, &hf_path).map(Some),
                    Err(e) => Err(e),
                }
            }
        } else {
            Ok(None)
        };

        let mmproj_path = match mmproj_result {
            Ok(p) => p,
            Err(e) => {
                println!("  Multimodal projector download failed: {e}");
                println!("  Try a different model.");
                println!();
                continue;
            }
        };

        // Load model
        println!();
        println!("  Loading model...");
        let engine = tokio::task::spawn_blocking({
            let model_path = model_path.clone();
            let mmproj_path = mmproj_path.clone();
            move || InferenceEngine::load(&model_path, gpu_layers, mmproj_path.as_deref())
        })
        .await
        .map_err(|e| NodeError::Inference(format!("task join error: {e}")))?;

        let engine = match engine {
            Ok(e) => e,
            Err(e) => {
                println!("  Failed to load model: {e}");
                println!("  Try a different model or quantization.");
                println!();
                continue;
            }
        };

        // Run test inference
        println!("  Running test inference...");
        println!();

        let model_name_owned = model_name.clone();
        let result = tokio::task::spawn_blocking(move || {
            let prompt = engine
                .apply_template(&[dkn_protocol::ChatMessage {
                    role: "user".into(),
                    content: dkn_protocol::MessageContent::Text("Hello!".into()),
                }])
                .unwrap_or_else(|_| "Hello!".into());

            let params = GenerateParams {
                max_tokens: 64,
                temperature: 0.7,
                ..Default::default()
            };

            print!("  > ");
            let result = engine.generate(&prompt, &params, |token| {
                print!("{}", token.text);
                let _ = io::stdout().flush();
                ControlFlow::Continue(())
            });
            println!();

            result.map(|r| (r, model_name_owned))
        })
        .await
        .map_err(|e| NodeError::Inference(format!("task join error: {e}")))?;

        match result {
            Ok((inference_result, name)) => {
                println!();
                println!(
                    "  Model working! {:.1} tok/s",
                    inference_result.tokens_per_second
                );
                break (spec, name, quant_override);
            }
            Err(e) => {
                println!("  Inference test failed: {e}");
                println!("  Try a different model.");
                println!();
                continue;
            }
        }
    };

    println!();
    println!("  To start the node:");
    if let Some(q) = quant_override {
        println!(
            "    dria-node start --wallet <YOUR_SECRET_KEY> --model {} --quant {}",
            model_name_final, q
        );
    } else {
        println!(
            "    dria-node start --wallet <YOUR_SECRET_KEY> --model {}",
            model_name_final
        );
    }
    println!();

    // Suppress unused variable warning
    let _ = spec;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_ram_returns_something() {
        // On CI / local machines this should return Some on Linux/macOS/Windows
        let ram = detect_ram_bytes();
        if cfg!(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "windows"
        )) {
            assert!(ram.is_some(), "should detect RAM on this platform");
            assert!(ram.unwrap() > 0);
        }
    }

    #[test]
    fn test_extract_quant() {
        assert_eq!(extract_quant("Qwen3.5-9B-Q4_K_M.gguf"), "Q4_K_M");
        assert_eq!(extract_quant("LFM2.5-VL-1.6B-Q4_0.gguf"), "Q4_0");
        assert_eq!(extract_quant("model.gguf"), "model");
    }

    #[test]
    fn test_model_size_known() {
        assert!(model_size_gb("lfm2.5:1.2b").is_some());
        assert!(model_size_gb("qwen3.5:9b").is_some());
        assert!(model_size_gb("nonexistent:1b").is_none());
    }

    #[test]
    fn test_model_size_ordering() {
        // RAM needed should always be >= size
        for name in [
            "lfm2.5:1.2b",
            "nanbeige:3b",
            "locooperator:4b",
            "lfm2.5-vl:1.6b",
            "lfm2.5-audio:1.5b",
            "qwen3.5:9b",
            "qwen3.5:27b",
            "qwen3.5:35b-a3b",
            "lfm2:24b-a2b",
        ] {
            let (size, needed) = model_size_gb(name).unwrap();
            assert!(
                needed >= size,
                "{name}: ram_needed ({needed}) should be >= size ({size})"
            );
        }
    }

    #[test]
    fn test_all_registry_models_have_sizes() {
        let registry = default_registry();
        for name in registry.keys() {
            assert!(
                model_size_gb(name).is_some(),
                "missing size estimate for registry model: {name}"
            );
        }
    }
}
