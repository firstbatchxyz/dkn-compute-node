use dkn_executor::{DriaExecutorsManager, Model};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    dotenvy::dotenv().ok();

    let model = Model::GPT4o;
    let models = vec![model];
    let mut config = DriaExecutorsManager::new_from_env_for_models(models.into_iter())?;
    config.check_services().await;
    assert!(config.models.contains(&model));

    let task = dkn_executor::TaskBody::new_prompt("Write a haiku about category theory.", model);
    let executor = config.get_executor(&task.model).await?;
    let result = executor.execute(task).await?;

    println!("{}", result);
    Ok(())
}
