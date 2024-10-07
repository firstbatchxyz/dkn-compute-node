use std::env;

use dkn_workflows::ModelConfig;
use eyre::Result;
use ollama_workflows::Model;

#[tokio::test]
#[ignore = "requires Ollama"]
async fn test_ollama() -> Result<()> {
    env::set_var("RUST_LOG", "none,dkn_workflows=debug");
    let _ = env_logger::try_init();

    let models = vec![Model::Phi3_5Mini];
    let mut model_config = ModelConfig::new(models);

    model_config.check_services().await
}

#[tokio::test]
async fn test_openai() -> Result<()> {
    env::set_var("RUST_LOG", "debug");
    let _ = env_logger::try_init();

    let models = vec![Model::GPT4Turbo];
    let mut model_config = ModelConfig::new(models);

    model_config.check_services().await
}

#[tokio::test]
async fn test_empty() -> Result<()> {
    let mut model_config = ModelConfig::new(vec![]);

    let result = model_config.check_services().await;
    assert!(result.is_err());

    Ok(())
}
