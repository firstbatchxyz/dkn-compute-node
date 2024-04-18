#[cfg(feature = "ollama_test")]
mod ollama_tests {
    use ollama_rs::{generation::completion::request::GenerationRequest, Ollama};

    #[tokio::test]
    async fn test_ollama_prompt() {
        let ollama = Ollama::default();
        let model = "orca-mini:latest".to_string(); // very small model
        let prompt = "Why is the sky blue?".to_string();

        // let res: ollama_rs::models::pull::PullModelStatus = ollama.pull_model(model, false).await.unwrap();

        let res = ollama
            .generate(GenerationRequest::new(model, prompt))
            .await
            .unwrap();
        println!("{:?}", res.response);
    }
}
