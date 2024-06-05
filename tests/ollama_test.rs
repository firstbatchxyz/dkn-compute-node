#[cfg_attr(test, cfg(feature = "ollama_test"))]
mod ollama_test {
    use langchain_rust::{language_models::llm::LLM, llm::client::Ollama};

    #[tokio::test]
    async fn test_ollama_prompt() {
        let model = "orca-mini".to_string();
        let ollama = Ollama::default().with_model(model);
        let prompt = "The sky appears blue during the day because of a process called scattering. \
                When sunlight enters the Earth's atmosphere, it collides with air molecules such as oxygen and nitrogen. \
                These collisions cause some of the light to be absorbed or reflected, which makes the colors we see appear more vivid and vibrant. \
                Blue is one of the brightest colors that is scattered the most by the atmosphere, making it visible to our eyes during the day. \
                What may be the question this answer?".to_string();

        let response = ollama
            .invoke(&prompt)
            .await
            .expect("Should generate response");
        println!("Prompt: {}\n\nResponse:{}", prompt, response);
    }

    // TODO: move to pull model test
    // #[tokio::test]
    // async fn test_ollama_bad_model() {
    //     let model = "thismodeldoesnotexistlol".to_string();
    //     let ollama = OllamaClient::new(None, None, Some(model));

    //     let setup_res = ollama.setup(CancellationToken::default()).await;
    //     assert!(
    //         setup_res.is_err(),
    //         "Should give error due to non-existing model."
    //     );
    // }
}
