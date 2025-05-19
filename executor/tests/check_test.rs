use dkn_executor::{DriaExecutorsConfig, Model};
use eyre::Result;

fn setup() {
    // read api key from .env
    let _ = dotenvy::dotenv();

    // set logger
    let _ = env_logger::builder()
        .filter_level(log::LevelFilter::Off)
        .filter_module("models_test", log::LevelFilter::Debug)
        .filter_module("dkn_executor", log::LevelFilter::Debug)
        .is_test(true)
        .try_init();
}

#[tokio::test]
#[ignore = "requires Ollama"]
async fn test_ollama_check() -> Result<()> {
    setup();

    let models = vec![Model::Llama3_2_1bInstructQ4Km];
    let mut model_config = DriaExecutorsConfig::new_from_env_for_models(models)?;
    model_config.check_services().await?;

    assert!(model_config
        .models
        .contains(&Model::Llama3_2_1bInstructQ4Km));
    Ok(())
}

#[tokio::test]
#[ignore = "requires OpenAI"]
async fn test_openai_check() -> Result<()> {
    setup();

    let models = vec![Model::GPT4o];
    let mut model_config = DriaExecutorsConfig::new_from_env_for_models(models)?;
    model_config.check_services().await?;

    assert!(model_config.models.contains(&Model::GPT4o));
    Ok(())
}

#[tokio::test]
#[ignore = "requires Gemini"]
async fn test_gemini_check() -> Result<()> {
    setup();

    let models = vec![Model::Gemini2_0Flash];
    let mut model_config = DriaExecutorsConfig::new_from_env_for_models(models)?;
    model_config.check_services().await?;

    assert!(model_config.models.contains(&Model::Gemini2_0Flash));
    Ok(())
}

#[tokio::test]
#[ignore = "requires OpenRouter"]
async fn test_openrouter_check() -> Result<()> {
    setup();

    let models = vec![Model::OR3_5Sonnet];
    let mut model_config = DriaExecutorsConfig::new_from_env_for_models(models)?;
    model_config.check_services().await?;

    assert!(model_config.models.contains(&Model::OR3_5Sonnet));
    Ok(())
}

#[tokio::test]
async fn test_empty() -> Result<()> {
    assert!(DriaExecutorsConfig::new_from_env_for_models(Vec::new())?
        .check_services()
        .await
        .is_err());

    Ok(())
}
