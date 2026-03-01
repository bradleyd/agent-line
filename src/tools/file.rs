use crate::agent::StepError;
use std::fs::OpenOptions;
use std::io::Write;

/// Read an entire file into a string.
pub fn read_file(path: &str) -> Result<String, StepError> {
    Ok(std::fs::read_to_string(path)?)
}
/// Write content to a file, creating parent directories if needed.
pub fn write_file(path: &str, content: &str) -> Result<(), StepError> {
    if let Some(parent) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(std::fs::write(path, content)?)
}

/// List entries in a directory.
pub fn list_dir(path: &str) -> Result<Vec<String>, StepError> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        entries.push(entry.path().display().to_string());
    }
    Ok(entries)
}
/// Recursively find files matching a suffix pattern (e.g. `"*.rs"`).
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

/// Append content to a file, creating it if it doesn't exist.
pub fn append_file(file_path: &str, content: &str) -> Result<(), StepError> {
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(file_path)?;

    file.write_all(content.as_bytes())?;
    Ok(())
}

/// Check if a file exists.
pub fn file_exists(file_path: &str) -> bool {
    std::path::Path::new(file_path).exists()
}

/// Delete a file.
pub fn delete_file(file_path: &str) -> Result<(), StepError> {
    std::fs::remove_file(file_path)?;
    Ok(())
}

/// Create a directory and all parent directories.
pub fn create_dir(name: &str) -> Result<(), StepError> {
    std::fs::create_dir_all(name)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_append_to_existing_file() {
        let path = "/tmp/agent_line_test_append.txt";
        let _ = std::fs::remove_file(path);
        write_file(path, "hello").unwrap();
        append_file(path, " world").unwrap();
        assert_eq!(read_file(path).unwrap(), "hello world");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_append_creates_file_if_missing() {
        let path = "/tmp/agent_line_test_append_new.txt";
        let _ = std::fs::remove_file(path);
        append_file(path, "new content").unwrap();
        assert_eq!(read_file(path).unwrap(), "new content");
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_file_exists_true() {
        let path = "/tmp/agent_line_test_exists.txt";
        write_file(path, "data").unwrap();
        assert!(file_exists(path));
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_file_exists_false() {
        assert!(!file_exists("/tmp/agent_line_nonexistent_xyz.txt"));
    }

    #[test]
    fn test_delete_file_removes_it() {
        let path = "/tmp/agent_line_test_delete.txt";
        write_file(path, "data").unwrap();
        delete_file(path).unwrap();
        assert!(!file_exists(path));
    }

    #[test]
    fn test_delete_file_not_found_is_error() {
        let result = delete_file("/tmp/agent_line_no_such_file_xyz.txt");
        assert!(result.is_err());
    }

    #[test]
    fn test_create_dir_simple() {
        let path = "/tmp/agent_line_test_dir";
        let _ = std::fs::remove_dir_all(path);
        create_dir(path).unwrap();
        assert!(std::path::Path::new(path).is_dir());
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn test_create_dir_nested() {
        let path = "/tmp/agent_line_test_dir_nested/a/b/c";
        let _ = std::fs::remove_dir_all("/tmp/agent_line_test_dir_nested");
        create_dir(path).unwrap();
        assert!(std::path::Path::new(path).is_dir());
        std::fs::remove_dir_all("/tmp/agent_line_test_dir_nested").unwrap();
    }
}
