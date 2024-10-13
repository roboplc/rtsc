use super::Params;
use crate::{Error, Result};

pub fn apply_for_current(_params: &Params) -> Result<()> {
    Err(Error::Unimplemented)
}

pub fn apply(_tid: libc::c_int, _params: &Params) -> Result<()> {
    Err(Error::Unimplemented)
}

pub fn prealloc_heap(size: usize) -> Result<()> {
    if size == 0 {
        return Ok(());
    }
    Err(Error::Unimplemented)
}
