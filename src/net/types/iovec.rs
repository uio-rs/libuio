use nix::libc;

use crate::ptr::SendMut;

#[repr(C)]
pub struct IoVec {
    pub iov_base: SendMut<libc::c_void>,
    pub iov_len: libc::size_t,
}
