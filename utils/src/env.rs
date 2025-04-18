/// Reads an environment variable and trims whitespace and `"` from both ends.
/// If the trimmed value is empty, returns `None`.
#[inline]
pub fn safe_read_env(var: Result<String, std::env::VarError>) -> Option<String> {
    var.map(|s| s.trim_matches('"').trim().to_string())
        .ok()
        .filter(|s| !s.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_var_read() {
        let var = Ok("\"  value  \"".to_string());
        assert_eq!(safe_read_env(var), Some("value".to_string()));

        let var = Ok("\"  \"".to_string());
        assert!(safe_read_env(var).is_none());

        let var = Err(std::env::VarError::NotPresent);
        assert!(safe_read_env(var).is_none());
    }
}
