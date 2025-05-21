use dkn_executor::{DriaExecutorsManager, Model};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    dotenvy::dotenv().ok();

    let model = Model::Gemini2_0Flash;
    let models = vec![model];
    let mut config = DriaExecutorsManager::new_from_env_for_models(models)?;
    config.check_services().await?;

    assert!(config.models.contains(&model));

    // make a request
    let task = dkn_executor::TaskBody::new_prompt("Write a haiku about category theory.", model);
    let executor = config
        .get_executor(&task.model)
        .await
        .expect("could not get executor");
    let result = executor
        .execute(task)
        .await
        .expect("failed to execute task");

    println!("{}", result);
    Ok(())
}
