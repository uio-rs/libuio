use nix::libc;

use crate::ptr::SendMut;

use super::IoVec;

#[repr(C)]
pub struct MsgHdr {
    pub msg_name: SendMut<libc::c_void>,
    pub msg_namelen: libc::socklen_t,
    pub msg_iov: SendMut<IoVec>,
    pub msg_iovlen: libc::size_t,
    pub msg_control: SendMut<libc::c_void>,
    pub msg_controllen: libc::size_t,
    pub msg_flags: libc::c_int,
}

impl MsgHdr {
    pub fn as_mut_ptr(&mut self) -> *mut libc::msghdr {
        self as *mut _ as *mut libc::msghdr
    }
}
