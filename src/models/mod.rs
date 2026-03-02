pub mod cache;
pub mod download;
pub mod registry;

pub mod template {
    pub use dkn_protocol::{apply_chat_template, ChatMessage};
}

pub use cache::ModelCache;
pub use download::ModelDownloader;
pub use registry::{default_registry, resolve_model};
