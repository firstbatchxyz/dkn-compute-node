pub const DEFAULT_DKN_WAKU_URL: &str = "http://127.0.0.1:8645";

/// 32 byte secret key hex(b"node") * 8
pub const DEFAULT_DKN_WALLET_SECRET_KEY: &str =
    "6e6f64656e6f64656e6f64656e6f64656e6f64656e6f64656e6f64656e6f6465";

/// 33 byte compressed public key of secret key from hex(b"dria) * 8
pub const DEFAULT_DKN_ADMIN_PUBLIC_KEY: &str =
    "0208ef5e65a9c656a6f92fb2c770d5d5e2ecffe02a6aade19207f75110be6ae658";

pub const DEFAULT_DKN_HEARTBEAT_TIMEOUT: &str = "1000"; // millis

pub const DEFAULT_DKN_OLLAMA_HOST: &str = "http://127.0.0.1";

pub const DEFAULT_DKN_OLLAMA_PORT: &str = "11434";
