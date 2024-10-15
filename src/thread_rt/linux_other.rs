use super::{Params, Scheduling};
use crate::{Error, Result};
use std::process::Stdio;

struct ChrtSchedArgument(&'static str);

impl From<Scheduling> for ChrtSchedArgument {
    fn from(value: Scheduling) -> Self {
        match value {
            Scheduling::RoundRobin => ChrtSchedArgument("--rr"),
            Scheduling::FIFO => ChrtSchedArgument("--fifo"),
            Scheduling::Idle => ChrtSchedArgument("--idle"),
            Scheduling::Batch => ChrtSchedArgument("--batch"),
            Scheduling::DeadLine => ChrtSchedArgument("--deadline"),
            Scheduling::Other => ChrtSchedArgument("--other"),
        }
    }
}

pub fn apply_for_current(params: &Params) -> Result<()> {
    let tid = unsafe { i32::try_from(libc::syscall(libc::SYS_gettid)) }
        .map_err(|e| Error::Failed(e.to_string()))?;
    apply(tid, params)
}

pub fn apply(tid: libc::c_int, params: &Params) -> Result<()> {
    let user_id = unsafe { libc::getuid() };
    if !params.cpu_ids.is_empty() {
        if user_id != 0 {
            return Err(Error::AccessDenied);
        }
        let result = std::process::Command::new("taskset")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .arg("-cp")
            .arg(
                &params
                    .cpu_ids
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            )
            .arg(tid.to_string())
            .status()
            .map_err(|e| Error::Failed(format!("CPU affinity set error (taskset): {}", e)))?;
        if !result.success() {
            return Err(Error::Failed(format!(
                "CPU affinity set error (taskset): exit code {}",
                result.code().unwrap_or(-1)
            )));
        }
    }
    if let Some(priority) = params.priority {
        if user_id != 0 {
            return Err(Error::AccessDenied);
        }
        let sched = if priority == 0 {
            Scheduling::Other
        } else {
            params.scheduling
        };
        let result = std::process::Command::new("chrt")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .arg(ChrtSchedArgument::from(sched).0)
            .arg("-p")
            .arg(priority.to_string())
            .arg(tid.to_string())
            .status()
            .map_err(|e| Error::Failed(format!("Real-time priority set error (chrt): {}", e)))?;
        if !result.success() {
            return Err(Error::Failed(format!(
                "Real-time priority set error (chrt): exit code {}",
                result.code().unwrap_or(-1)
            )));
        }
    }
    Ok(())
}

pub fn prealloc_heap(size: usize) -> Result<()> {
    if size == 0 {
        return Ok(());
    }
    Err(Error::Unimplemented)
}
