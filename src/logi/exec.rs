use std::path::PathBuf;
use std::process::Command;

use crate::logi::{gp::Group, mem::Member};

/// Result of running a script inside one member (cabinet) root.
#[derive(Debug, Clone)]
pub struct ExecOutcome {
    pub cabinet_abs_path: PathBuf,
    pub success: bool,
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub error: Option<String>,
}

impl Group {
    pub fn exec(&self, script: &String) -> Vec<ExecOutcome> {
        self.mems.iter().map(|mem| mem.exec(script)).collect()
    }
}

impl Member {
    pub fn exec(&self, script: &String) -> ExecOutcome {
        let cd_dir = self.mem_info.cab_info.abs_path.as_path();

        // description: cd cd_dir & sh -c <Script>

        #[cfg(target_family = "unix")]
        let output = {
            let mut cmd = Command::new("sh");
            cmd.arg("-c").arg(script).current_dir(cd_dir);
            cmd.output()
        };

        #[cfg(target_family = "windows")]
        let output = {
            let mut cmd = Command::new("cmd");
            cmd.arg("/C").arg(script).current_dir(cd_dir);
            cmd.output()
        };

        match output {
            Ok(out) => ExecOutcome {
                cabinet_abs_path: self.mem_info.cab_info.abs_path.clone(),
                success: out.status.success(),
                code: out.status.code(),
                stdout: String::from_utf8_lossy(&out.stdout).to_string(),
                stderr: String::from_utf8_lossy(&out.stderr).to_string(),
                error: None,
            },
            Err(e) => ExecOutcome {
                cabinet_abs_path: self.mem_info.cab_info.abs_path.clone(),
                success: false,
                code: None,
                stdout: String::new(),
                stderr: String::new(),
                error: Some(format!("failed to execute script: {}", e)),
            },
        }
    }
}
