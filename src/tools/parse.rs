use crate::agent::StepError;

pub fn strip_code_fences(response: &str) -> String {
    let trimmed = response.trim();
    if trimmed.starts_with("```") {
        let lines: Vec<&str> = trimmed.lines().collect();
        // Skip first line (```rust) and last line (```)
        lines[1..lines.len() - 1].join("\n")
    } else {
        trimmed.to_string()
    }
}

pub fn parse_lines(response: &str) -> Vec<String> {
    response
        .lines() // split into individual lines
        .map(|line| line.trim().trim_start_matches(|c: char| c.is_ascii_digit()))
        .map(|line| line.strip_prefix(".").unwrap_or(line))
        .map(|line| line.strip_prefix("-").unwrap_or(line))
        .map(|line| line.strip_prefix("*").unwrap_or(line))
        .map(|line| line.trim())
        .map(|line| line.to_string())
        .filter(|line| !line.is_empty())
        .collect()
}

pub fn extract_json(response: &str) -> Result<String, StepError> {
    let no_fences = strip_code_fences(response);
    // get opening { or  [
    let trimmed = no_fences.trim();
    let index = trimmed.find(|c| c == '{' || c == '[');
    if let Some(start) = index {
        let slice = &trimmed[start..];
        let parsed = serde_json::Deserializer::from_str(slice)
            .into_iter::<serde_json::Value>()
            .next();

        if let Some(Ok(val)) = parsed {
            return Ok(val.to_string());
        }
    }

    Err(StepError::Invalid("invalid json".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- parse_lines tests ---

    #[test]
    fn test_parse_lines_numbered() {
        let input = "1. First item\n2. Second item\n3. Third item";
        let result = parse_lines(input);
        assert_eq!(result, vec!["First item", "Second item", "Third item"]);
    }

    #[test]
    fn test_parse_lines_dashes() {
        let input = "- Alpha\n- Beta\n- Gamma";
        let result = parse_lines(input);
        assert_eq!(result, vec!["Alpha", "Beta", "Gamma"]);
    }

    #[test]
    fn test_parse_lines_asterisks() {
        let input = "* One\n* Two";
        let result = parse_lines(input);
        assert_eq!(result, vec!["One", "Two"]);
    }

    #[test]
    fn test_parse_lines_plain() {
        let input = "First\nSecond\nThird";
        let result = parse_lines(input);
        assert_eq!(result, vec!["First", "Second", "Third"]);
    }

    #[test]
    fn test_parse_lines_skips_empty_lines() {
        let input = "One\n\nTwo\n\n";
        let result = parse_lines(input);
        assert_eq!(result, vec!["One", "Two"]);
    }

    #[test]
    fn test_parse_lines_trims_whitespace() {
        let input = "  1. Padded  \n  2. Also padded  ";
        let result = parse_lines(input);
        assert_eq!(result, vec!["Padded", "Also padded"]);
    }

    // --- extract_json tests ---

    #[test]
    fn test_extract_json_object_from_prose() {
        let input = "Here is the result:\n{\"name\": \"test\", \"value\": 42}\nDone.";
        let result = extract_json(input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["name"], "test");
        assert_eq!(parsed["value"], 42);
    }

    #[test]
    fn test_extract_json_array() {
        let input = "The topics are: [\"rust\", \"python\", \"go\"]";
        let result = extract_json(input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed[0], "rust");
    }

    #[test]
    fn test_extract_json_in_code_fence() {
        let input = "```json\n{\"key\": \"value\"}\n```";
        let result = extract_json(input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["key"], "value");
    }

    #[test]
    fn test_extract_json_no_json_returns_error() {
        let input = "There is no JSON here at all.";
        let result = extract_json(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_json_nested_object() {
        let input = "Result: {\"outer\": {\"inner\": true}}";
        let result = extract_json(input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["outer"]["inner"], true);
    }
}
