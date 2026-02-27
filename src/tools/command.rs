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
