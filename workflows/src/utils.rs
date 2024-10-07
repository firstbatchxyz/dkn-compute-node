/// Utility to parse comma-separated string values, mostly read from the environment.
///
/// - Trims `"` from both ends at the start
/// - For each item, trims whitespace from both ends
pub fn split_comma_separated(input: &str) -> Vec<String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_example() {
        // should ignore whitespaces and `"` at both ends, and ignore empty items
        let input = "\"a,    b , c ,,  \"";
        let expected = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        assert_eq!(split_comma_separated(input), expected);
    }

    #[test]
    fn test_empty() {
        assert!(split_comma_separated(Default::default()).is_empty());
    }
}
