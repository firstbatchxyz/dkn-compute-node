use hex_literal::hex;

// DKN
pub const DKN_ADMIN_PUBLIC_KEY: &str = "DKN_ADMIN_PUBLIC_KEY";
pub const DKN_WALLET_SECRET_KEY: &str = "DKN_WALLET_SECRET_KEY";
pub const DKN_WALLET_PUBLIC_KEY: &str = "DKN_WALLET_PUBLIC_KEY";
pub const DKN_WALLET_ADDRESS: &str = "DKN_WALLET_ADDRESS";
pub const DKN_TASKS: &str = "DKN_TASKS";
pub const DKN_SYNTHESIS_LLM_TYPE: &str = "DKN_SYNTHESIS_LLM_TYPE";
/// 33 byte compressed public key of secret key from hex(b"dria) * 8, dummy only
pub const DEFAULT_DKN_ADMIN_PUBLIC_KEY: &[u8; 33] =
    &hex!("0208ef5e65a9c656a6f92fb2c770d5d5e2ecffe02a6aade19207f75110be6ae658");

/// 32 byte secret key hex(b"node") * 8, dummy only
pub const DEFAULT_DKN_WALLET_SECRET_KEY: &[u8; 32] =
    &hex!("6e6f64656e6f64656e6f64656e6f64656e6f64656e6f64656e6f64656e6f6465");

// Ollama
pub const OLLAMA_HOST: &str = "OLLAMA_HOST";
pub const OLLAMA_PORT: &str = "OLLAMA_PORT";
pub const OLLAMA_MODEL: &str = "OLLAMA_MODEL";
pub const DEFAULT_OLLAMA_HOST: &str = "http://127.0.0.1";
pub const DEFAULT_OLLAMA_PORT: u16 = 11434;
pub const DEFAULT_OLLAMA_MODEL: &str = "phi3";

// OpenAI
pub const OPENAI_API_BASE_URL: &str = "OPENAI_API_BASE_URL";
pub const OPENAI_API_KEY: &str = "OPENAI_API_KEY";
pub const OPENAI_ORG_ID: &str = "OPENAI_ORG_ID";
pub const OPENAI_PROJECT_ID: &str = "OPENAI_PROJECT_ID";

// Search Agent (Python)
pub const SEARCH_AGENT_URL: &str = "SEARCH_AGENT_URL";
pub const SEARCH_AGENT_MANAGER: &str = "SEARCH_AGENT_MANAGER";
