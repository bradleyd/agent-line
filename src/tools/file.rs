use crate::agent::StepError;

pub fn read_file(path: &str) -> Result<String, StepError> {
    Ok(std::fs::read_to_string(path)?)
}
pub fn write_file(path: &str, content: &str) -> Result<(), StepError> {
    if let Some(parent) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(std::fs::write(path, content)?)
}

pub fn list_dir(path: &str) -> Result<Vec<String>, StepError> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        entries.push(entry.path().display().to_string());
    }
    Ok(entries)
}
pub fn find_files(path: &str, pattern: &str) -> Result<Vec<String>, StepError> {
    let mut results = Vec::new();
    find_files_recursive(path, pattern, &mut results)?;
    Ok(results)
}

fn find_files_recursive(
    dir: &str,
    pattern: &str,
    results: &mut Vec<String>,
) -> Result<(), StepError> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            find_files_recursive(&path.display().to_string(), pattern, results)?;
        } else if let Some(name) = path.file_name().and_then(|n| n.to_str())
            && name.ends_with(pattern.trim_start_matches('*'))
        {
            results.push(path.display().to_string());
        }
    }
    Ok(())
}
