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
