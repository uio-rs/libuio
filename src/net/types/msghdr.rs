use nix::libc;

#[derive(Debug)]
#[repr(C)]
pub struct MsgHdr {
    pub msg_name: *mut libc::c_void,
    pub msg_namelen: libc::socklen_t,
    pub msg_iov: *mut libc::iovec,
    pub msg_iovlen: libc::size_t,
    pub msg_control: *mut libc::c_void,
    pub msg_controllen: libc::size_t,
    pub msg_flags: libc::c_int,
}

impl MsgHdr {
    pub fn as_mut_ptr(&mut self) -> *mut libc::msghdr {
        self as *mut _ as *mut libc::msghdr
    }
}

unsafe impl Send for MsgHdr {}
