use dkn_workflows::{DriaWorkflowsConfig, Model};
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

    let models = vec![Model::Llama3_2_1bInstructQ4Km];
    let mut model_config = DriaWorkflowsConfig::new(models);
    model_config.check_services().await?;

    assert_eq!(model_config.models[0], Model::Llama3_2_1bInstructQ4Km);

    Ok(())
}

#[tokio::test]
#[ignore = "requires OpenAI"]
async fn test_openai_check() -> Result<()> {
    setup();

    let models = vec![Model::GPT4o];
    let mut model_config = DriaWorkflowsConfig::new(models);
    model_config.check_services().await?;

    assert_eq!(model_config.models[0], Model::GPT4o);
    Ok(())
}

#[tokio::test]
#[ignore = "requires Gemini"]
async fn test_gemini_check() -> Result<()> {
    setup();

    let models = vec![Model::Gemini2_0Flash];
    let mut model_config = DriaWorkflowsConfig::new(models);
    model_config.check_services().await?;

    assert_eq!(model_config.models[0], Model::Gemini2_0Flash);
    Ok(())
}

#[tokio::test]
#[ignore = "requires OpenRouter"]
async fn test_openrouter_check() -> Result<()> {
    setup();

    let models = vec![Model::OR3_5Sonnet];
    let mut model_config = DriaWorkflowsConfig::new(models);
    model_config.check_services().await?;

    assert_eq!(model_config.models[0], Model::OR3_5Sonnet);
    Ok(())
}

#[tokio::test]
async fn test_empty() {
    assert!(DriaWorkflowsConfig::new(vec![])
        .check_services()
        .await
        .is_err());
}
