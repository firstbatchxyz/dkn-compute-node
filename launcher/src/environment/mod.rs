//! This module is used to load environment variables from a `.env` file into their respective structs.
//!
//! It makes use of `dotenv_config::EnvConfig` to load the environment variables into the struct.
//! For a struct `foo` and field `bar`, this module will look for an environment variable `FOO_BAR`, unless a `name` attribute is provided
//! such as `env_config(name = "BAZ")` in which case it will look for `BAZ`.

mod apikeys;
mod ollama;
