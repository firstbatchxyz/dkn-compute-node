/// Within Waku Message and Content Topic we specify version to be 0 since
///  encryption takes place at our application layer, instead of at protocol layer of Waku.
pub const WAKU_ENC_VERSION: u8 = 0;

/// Within Content Topic we specify encoding to be `proto` as is the recommendation by Waku.
pub const WAKU_ENCODING: &str = "proto";

/// App-name for the Content Topic.
pub const WAKU_APP_NAME: &str = "dria";

/// Topic name for the heartbeat check.
pub const WAKU_HEARTBEAT_TOPIC: &str = "heartbeat";
