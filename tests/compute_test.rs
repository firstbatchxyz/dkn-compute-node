#![allow(unused_imports)]

mod compute_test {
    use dkn_compute::compute::{llm::ollama::create_ollama, search_python::SearchPythonClient};
    use langchain_rust::{language_models::llm::LLM, llm::client::Ollama};
    use std::env;
    use tokio_util::sync::CancellationToken;

    #[cfg_attr(test, cfg(feature = "search_python_test"))]
    #[tokio::test]
    #[ignore = "run this manually"]
    async fn test_search_python() {
        env::set_var("RUST_LOG", "INFO");
        let _ = env_logger::try_init();
        let search_client = SearchPythonClient::new();

        let result = search_client
            .search("Who is the president of the United States?".to_string())
            .await
            .expect("should search");
        println!("Result: {:?}", result);
    }

    #[cfg_attr(test, cfg(feature = "ollama_test"))]
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

    #[cfg_attr(test, cfg(feature = "ollama_test"))]
    #[tokio::test]
    async fn test_ollama_bad_model() {
        let model = "thismodeldoesnotexistlol".to_string();
        let setup_res = create_ollama(CancellationToken::default(), model).await;
        assert!(
            setup_res.is_err(),
            "Should give error due to non-existing model."
        );
    }
}
