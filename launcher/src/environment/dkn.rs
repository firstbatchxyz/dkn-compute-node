use dotenv_config::EnvConfig;

#[derive(Debug, Default, EnvConfig)]
struct DKN {
    wallet_secret_key: String,
    #[env_config(name = "DKN_P2P_LISTEN_ADDR", default = "/ip4/0.0.0.0/tcp/4001")]
    p2p_listen_addr: String,
    // #[env_config(default = 5)]
    batch_size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_reader() {
        assert!(dotenvy::dotenv().is_ok());
        let cfg = DKN::init().unwrap();
        println!("{:#?}", cfg);
    }
}
