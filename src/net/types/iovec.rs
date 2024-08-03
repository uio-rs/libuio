use nix::libc;

#[repr(C)]
pub struct IoVec {
    pub iov_base: *mut libc::c_void,
    pub iov_len: libc::size_t,
}

impl IoVec {
    pub fn as_mut_ptr(&mut self) -> *mut libc::iovec {
        self as *mut _ as *mut libc::iovec
    }
}

unsafe impl Send for IoVec {}
