use std::{fmt::Debug, str::FromStr, time::SystemTime};

/// Utility to parse comma-separated string value line.
///
/// - Trims `"` from both ends for the input
/// - For each item, trims whitespace from both ends
pub fn split_csv_line(input: &str) -> Vec<String> {
    input
        .trim_matches('"')
        .split(',')
        .filter_map(|s| {
            let s = s.trim().to_string();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        })
        .collect::<Vec<_>>()
}

/// Reads an environment variable and trims whitespace and `"` from both ends.
/// If the trimmed value is empty, returns `None`.
#[inline]
pub fn safe_read_env(var: Result<String, std::env::VarError>) -> Option<String> {
    var.map(|s| s.trim_matches('"').trim().to_string())
        .ok()
        .filter(|s| !s.is_empty())
}

/// Like `parse` of `str` but for vectors.
pub fn parse_vec<T>(input: Vec<impl AsRef<str> + Debug>) -> Result<Vec<T>, T::Err>
where
    T: FromStr,
{
    let parsed = input
        .iter()
        .map(|s| s.as_ref().parse::<T>())
        .collect::<Result<Vec<_>, _>>()?;

    Ok(parsed)
}

/// Returns the current time in nanoseconds since the Unix epoch.
///
/// If a `SystemTimeError` occurs, will return 0 just to keep things running.
#[inline]
pub fn get_current_time_nanos() -> u128 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_abc_csv() {
        // should ignore whitespaces and `"` at both ends, and ignore empty items
        let input = "\"a,    b , c ,,  \"";
        let expected = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        assert_eq!(split_csv_line(input), expected);
    }

    #[test]
    fn test_empty_csv() {
        assert!(split_csv_line(Default::default()).is_empty());
    }

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
