use std::{collections::HashMap, io, path::PathBuf};

#[derive(Debug, Clone)]
pub struct DriaEnv {
    kv: HashMap<&'static str, String>,
    is_changed: bool,
}

impl DriaEnv {
    /// All environment keys that we are interested in.
    pub const KEY_NAMES: [&str; 12] = [
        // DKN
        "DKN_WALLET_SECRET_KEY",
        "DKN_MODELS",
        "DKN_P2P_LISTEN_ADDR",
        "DKN_BATCH_SIZE",
        // API keys
        "OPENAI_API_KEY",
        "GEMINI_API_KEY",
        "OPENROUTER_API_KEY",
        "SERPER_API_KEY",
        "JINA_API_KEY",
        // Ollama
        "OLLAMA_HOST",
        "OLLAMA_PORT",
        "OLLAMA_AUTO_PULL",
    ];

    pub fn is_changed(&self) -> bool {
        self.is_changed
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.kv.get(key).map(|s| s.as_str())
    }

    pub fn set(&mut self, key: &'static str, value: String) {
        self.kv.insert(key, value);
        self.is_changed = true;
    }

    pub fn new() -> Self {
        Self {
            kv: HashMap::from_iter(
                Self::KEY_NAMES
                    .into_iter()
                    .filter_map(|k| std::env::var(k).map(|v| (k, v)).ok()),
            ),
            is_changed: false,
        }
    }

    /// Expects a content string (from an env file) and saves the keys to this content.
    ///
    /// - If a key exists in the content, it will be replaced with the value from the env.
    /// - If multiple keys exists for the same key name, only the last & uncommented one will be used.
    /// - If a key does not exist in the content, it will be appended to the end of the content.
    pub fn save_to_content(&self, content: &str) -> String {
        let mut ans_lines = Vec::<String>::new();
        let mut kv_to_add = self.kv.clone();

        for lines in content.lines() {
            // get keys via `iter_mut` because we cant remove them otherwise
            if let Some(matched_key) = kv_to_add
                .iter_mut()
                .map(|(k, _)| *k)
                .find(|k| lines.starts_with(&format!("{}=", k)))
            {
                // replace the line with the new value
                ans_lines.push(format!(
                    "{}={}",
                    matched_key,
                    kv_to_add.remove(matched_key).unwrap()
                ));
            } else {
                // ignore this line by adding it as is
                ans_lines.push(lines.to_string());
            }
        }

        for (k, v) in &kv_to_add {
            ans_lines.push(format!("{}={}", k, v));
        }

        ans_lines.join("\n")
    }

    /// Saves the environment to a file by adding the changes.
    pub fn save_to_file(&self, env_path: &PathBuf) -> io::Result<()> {
        println!("Saving changes to {}", env_path.display());

        let content = std::fs::read_to_string(env_path)?;
        let new_content = self.save_to_content(&content);

        std::fs::write(env_path, new_content)?;
        println!("Changes saved successfully.");
        Ok(())
    }
}

impl std::fmt::Display for DriaEnv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (k, v) in &self.kv {
            writeln!(f, "{}={}", k, v)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_providers() {
        dotenvy::dotenv().unwrap();
        let e = DriaEnv::new();
        println!("{}", e);
    }
}
