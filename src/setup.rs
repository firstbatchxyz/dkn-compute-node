use std::path::PathBuf;

use crate::error::NodeError;

/// Detect total system RAM in bytes.
fn detect_ram_bytes() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        if let Ok(contents) = std::fs::read_to_string("/proc/meminfo") {
            for line in contents.lines() {
                if let Some(rest) = line.strip_prefix("MemTotal:") {
                    let rest = rest.trim();
                    if let Some(kb_str) = rest.strip_suffix("kB").or_else(|| rest.strip_suffix("KB")) {
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
        let output = std::process::Command::new("sysctl").args(["-n", "hw.memsize"]).output().ok()?;
        String::from_utf8_lossy(&output.stdout).trim().parse::<u64>().ok()
    }

    #[cfg(target_os = "windows")]
    {
        let output = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", "(Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory"])
            .output()
            .ok()?;
        String::from_utf8_lossy(&output.stdout).trim().parse::<u64>().ok()
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        None
    }
}

/// Minimal setup for the all-Luminal GLM-5.2 node: the model is predetermined (no model
/// selection / GGUF download wizard), so setup just reports the machine and the start hint.
pub async fn run_setup(data_dir: Option<PathBuf>, _gpu_layers: i32) -> Result<(), NodeError> {
    println!();
    println!("  Welcome to Dria Node setup!");
    println!();

    if let Some(bytes) = detect_ram_bytes() {
        println!("  System: {:.0} GB RAM detected", bytes as f64 / (1024.0 * 1024.0 * 1024.0));
    }

    let data_dir = match data_dir {
        Some(d) => d,
        None => dirs::home_dir()
            .ok_or_else(|| NodeError::Config("could not determine home directory".into()))?
            .join(".dria"),
    };
    std::fs::create_dir_all(&data_dir)?;
    println!("  Data dir: {}", data_dir.display());
    println!();

    println!("  This node serves GLM-5.2 (pipeline-parallel, Luminal). The model is predetermined.");
    println!("  To start the node:");
    println!("    dria-node start --wallet <YOUR_SECRET_KEY> --router-url <ROUTER_URL>");
    println!();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_ram_returns_something() {
        let ram = detect_ram_bytes();
        if cfg!(any(target_os = "linux", target_os = "macos", target_os = "windows")) {
            assert!(ram.is_some(), "should detect RAM on this platform");
            assert!(ram.unwrap() > 0);
        }
    }
}
