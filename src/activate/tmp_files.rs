use super::ActivationResult;
use std::process;

type TmpFilesActivationResult = ActivationResult<process::ExitStatus>;

pub fn activate() -> TmpFilesActivationResult {
    let mut cmd = process::Command::new("systemd-tmpfiles");
    cmd.arg("--create");
    let status = cmd
        .stdout(process::Stdio::inherit())
        .stderr(process::Stdio::inherit())
        .status();
    Ok(status.unwrap())
}

pub fn deactivate() -> TmpFilesActivationResult {
    let mut cmd = process::Command::new("systemd-tmpfiles");
    cmd.arg("--clean").arg("--remove");
    let status = cmd
        .stdout(process::Stdio::inherit())
        .stderr(process::Stdio::inherit())
        .status();
    Ok(status.unwrap())
}
