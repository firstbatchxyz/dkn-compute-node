// TODO: env and config stuff
use std::env;

pub struct NodeConfig {
    DKN_WAKU_URL: String,
    DKN_WALLET_PRIVKEY: String,

    /// Milliseconds of timeout between each heartbeat message check.
    DKN_HEARTBEAT_TIMEOUT: u16,
}
