use std::process::Command;

use crate::agent::StepError;

pub struct CmdOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

pub fn run_cmd(cmd: &str) -> Result<CmdOutput, StepError> {
    let output = Command::new("sh").arg("-c").arg(cmd).output()?;

    Ok(CmdOutput {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

pub fn run_cmd_in_dir(dir_name: &str, cmd: &str) -> Result<CmdOutput, StepError> {
    let output = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .current_dir(dir_name)
        .output()?;

    Ok(CmdOutput {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_cmd_in_dir_uses_directory() {
        let output = run_cmd_in_dir("/tmp", "pwd").unwrap();
        assert!(output.success);
        // On macOS /tmp symlinks to /private/tmp
        let pwd = output.stdout.trim();
        assert!(pwd == "/tmp" || pwd == "/private/tmp");
    }

    #[test]
    fn test_run_cmd_in_dir_nonexistent_dir() {
        let result = run_cmd_in_dir("/nonexistent_dir_xyz_abc", "ls");
        assert!(result.is_err());
    }

    #[test]
    fn test_run_cmd_in_dir_runs_command() {
        let output = run_cmd_in_dir("/tmp", "echo hello").unwrap();
        assert!(output.success);
        assert_eq!(output.stdout.trim(), "hello");
    }
}
