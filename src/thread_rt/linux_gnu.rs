use super::{Params, Scheduling};
use crate::{Error, Result};

impl From<Scheduling> for libc::c_int {
    fn from(value: Scheduling) -> Self {
        match value {
            Scheduling::RoundRobin => libc::SCHED_RR,
            Scheduling::FIFO => libc::SCHED_FIFO,
            Scheduling::Idle => libc::SCHED_IDLE,
            Scheduling::Batch => libc::SCHED_BATCH,
            Scheduling::DeadLine => libc::SCHED_DEADLINE,
            Scheduling::Other => libc::SCHED_NORMAL,
        }
    }
}

pub fn apply_for_current(params: &Params) -> Result<()> {
    apply(0, params)
}

pub fn apply(tid: libc::c_int, params: &Params) -> Result<()> {
    let user_id = unsafe { libc::getuid() };
    if !params.cpu_ids.is_empty() {
        if user_id != 0 {
            return Err(Error::AccessDenied);
        }
        unsafe {
            let mut cpuset: libc::cpu_set_t = std::mem::zeroed();
            for cpu in &params.cpu_ids {
                libc::CPU_SET(*cpu, &mut cpuset);
            }
            let res = libc::sched_setaffinity(tid, std::mem::size_of::<libc::cpu_set_t>(), &cpuset);
            if res != 0 {
                return Err(Error::Failed(format!("CPU affinity set error: {}", res)));
            }
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
        let res = unsafe {
            libc::sched_setscheduler(
                tid,
                sched.into(),
                &libc::sched_param {
                    sched_priority: priority,
                },
            )
        };
        if res != 0 {
            return Err(Error::Failed(format!(
                "Real-time priority set error: {}",
                res
            )));
        }
    }
    Ok(())
}
