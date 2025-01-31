//! This module is used to load environment variables from a `.env` file into their respective structs.
//!
//! It makes use of `dotenv_config::EnvConfig` to load the environment variables into the struct.
//! For a struct `foo` and field `bar`, this module will look for an environment variable `FOO_BAR`, unless a `name` attribute is provided
//! such as `env_config(name = "BAZ")` in which case it will look for `BAZ`.
//!

use dotenv_config::EnvConfig;

#[derive(Debug, Default, EnvConfig)]
struct Ollama {
    #[env_config(default = "http://127.0.0.1")]
    host: String,
    #[env_config(default = 11434)]
    port: u16,
    #[env_config(default = true)]
    auto_pull: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config() {
        assert!(dotenvy::dotenv().is_ok());
        let cfg = Ollama::init().unwrap();
        println!("{:#?}", cfg);
    }
}
