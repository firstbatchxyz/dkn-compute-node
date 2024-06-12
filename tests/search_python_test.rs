#[cfg_attr(test, cfg(feature = "search_python_test"))]
mod search_python_test {
    use dkn_compute::compute::search_python::SearchPythonClient;

    #[tokio::test]
    #[ignore = "run this manually"]
    async fn test_search() {
        let _ = env_logger::try_init();
        let search_client = SearchPythonClient::new();

        let result = search_client
            .search("Who is the president of the United States?".to_string())
            .await
            .expect("should search");
        println!("Result: {:?}", result);
    }
}
