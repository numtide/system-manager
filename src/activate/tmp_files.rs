use crate::activate;

use super::ActivationResult;
use std::process;

type TmpFilesActivationResult = ActivationResult<()>;

pub fn activate() -> TmpFilesActivationResult {
    let mut cmd = process::Command::new("systemd-tmpfiles");
    cmd.arg("--create")
        .arg("--remove")
        .arg("/etc/tmpfiles.d/00-system-manager.conf");
    let output = cmd
        .stdout(process::Stdio::inherit())
        .stderr(process::Stdio::inherit())
        .output()
        .expect("Error forking process");

    output.status.success().then_some(()).ok_or_else(|| {
        activate::ActivationError::WithPartialResult {
            result: (),
            source: anyhow::anyhow!(
                "Error while creating tmpfiles\nstdout: {}\nstderr: {}",
                String::from_utf8_lossy(output.stdout.as_ref()),
                String::from_utf8_lossy(output.stderr.as_ref())
            ),
        }
    })
}
