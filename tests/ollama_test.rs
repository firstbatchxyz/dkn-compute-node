// #[cfg(feature = "ollama_test")]
mod ollama_tests {
    use ollama_rs::{generation::completion::request::GenerationRequest, Ollama};

    #[tokio::test]
    async fn test_ollama_prompt() {
        let ollama = Ollama::default();
        let model = "orca-mini:latest".to_string(); // very small model
        let prompt = "The sky appears blue during the day because of a process called scattering. When sunlight enters the Earth's atmosphere, it collides with air molecules such as oxygen and nitrogen. These collisions cause some of the light to be absorbed or reflected, which makes the colors we see appear more vivid and vibrant. Blue is one of the brightest colors that is scattered the most by the atmosphere, making it visible to our eyes during the day. What may be the question this answer?".to_string();

        // let res: ollama_rs::models::pull::PullModelStatus =
        //     ollama.pull_model(model, false).await.unwrap();
        // println!("{:?}", res);

        let res = ollama
            .generate(GenerationRequest::new(model, prompt))
            .await
            .unwrap();
        println!("{:?}", res.response);
    }
}
