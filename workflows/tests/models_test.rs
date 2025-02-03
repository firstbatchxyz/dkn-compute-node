use dkn_workflows::{DriaWorkflowsConfig, Model, ModelProvider};
use eyre::Result;

#[inline(always)]
fn setup() {
    // read api key from .env
    let _ = dotenvy::dotenv();

    // set logger
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Off)
        .filter_module("models_test", log::LevelFilter::Debug)
        .filter_module("dkn_workflows", log::LevelFilter::Debug)
        .is_test(true)
        .try_init();
}

#[tokio::test]
#[ignore = "requires Ollama"]
async fn test_ollama_check() -> Result<()> {
    setup();

    let models = vec![Model::Phi3_5Mini];
    let mut model_config = DriaWorkflowsConfig::new(models);
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
    setup();

    let models = vec![Model::GPT4Turbo];
    let mut model_config = DriaWorkflowsConfig::new(models);
    model_config.check_services().await?;

    assert_eq!(
        model_config.models[0],
        (ModelProvider::OpenAI, Model::GPT4Turbo)
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires Gemini"]
async fn test_gemini_check() -> Result<()> {
    setup();

    let models = vec![Model::Gemini15Flash];
    let mut model_config = DriaWorkflowsConfig::new(models);
    model_config.check_services().await?;

    assert_eq!(
        model_config.models[0],
        (ModelProvider::Gemini, Model::Gemini15Flash)
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires OpenRouter"]
async fn test_openrouter_check() -> Result<()> {
    setup();

    let models = vec![Model::ORDeepSeek2_5];
    let mut model_config = DriaWorkflowsConfig::new(models);
    model_config.check_services().await?;

    assert_eq!(
        model_config.models[0],
        (ModelProvider::OpenRouter, Model::ORDeepSeek2_5)
    );
    Ok(())
}

#[tokio::test]
async fn test_empty() {
    assert!(DriaWorkflowsConfig::new(vec![])
        .check_services()
        .await
        .is_err());
}
