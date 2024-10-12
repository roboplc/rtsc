use crate::{Error, Result};

/// Apply the thread scheduler and CPU affinity parameters for the current thread
pub fn apply_for_current(_params: &Params) -> Result<()> {
    Err(Error::Unimplemented)
}

pub fn apply(_tid: libc::c_int, _params: &Params) -> Result<()> {
    Err(Error::Unimplemented)
}
