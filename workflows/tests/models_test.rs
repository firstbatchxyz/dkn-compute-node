use std::env;

use dkn_workflows::{ModelConfig, ModelProvider};
use eyre::Result;
use ollama_workflows::Model;

const LOG_LEVEL: &str = "none,dkn_workflows=debug";

#[tokio::test]
#[ignore = "requires Ollama"]
async fn test_ollama_check() -> Result<()> {
    env::set_var("RUST_LOG", LOG_LEVEL);
    let _ = env_logger::try_init();

    let models = vec![Model::Phi3_5Mini];
    let mut model_config = ModelConfig::new(models);
    model_config.check_services().await?;

    assert_eq!(
        model_config.models[0],
        (ModelProvider::Ollama, Model::Phi3_5Mini)
    );

    Ok(())
}

#[tokio::test]
#[ignore = "requires OpenAI"]
async fn test_openai_check() -> Result<()> {
    let _ = dotenvy::dotenv(); // read api key
    env::set_var("RUST_LOG", LOG_LEVEL);
    let _ = env_logger::try_init();

    let models = vec![Model::GPT4Turbo];
    let mut model_config = ModelConfig::new(models);
    model_config.check_services().await?;

    assert_eq!(
        model_config.models[0],
        (ModelProvider::OpenAI, Model::GPT4Turbo)
    );
    Ok(())
}

#[tokio::test]
async fn test_empty() -> Result<()> {
    let mut model_config = ModelConfig::new(vec![]);

    let result = model_config.check_services().await;
    assert!(result.is_err());

    Ok(())
}
