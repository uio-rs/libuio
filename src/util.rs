/// The [SendConst] construct is used to ensure we tell the compiler that we can indeed send these
/// pointers around threads. Now clearly this is not normally something that you want to do,
/// however we have a hard requirement for this to work. This is due to how [io_uring] works under
/// the hood when it comes to buffers, you must pass in pointers to mutable memory regions for
/// reads and immutable memory regions for sends. To do this we need to rely on pinned heap memory,
/// and as such these pointers are safe to pass around to other threads, primarily via the
/// executors themselves as they execute async tasks.
#[repr(transparent)]
pub struct SendConst<T>(*const T);

impl<T> SendConst<T> {
    /// Create a new [SendConst] structure around a `*const T` of some kind, now there are some key
    /// safety considerations on using this function.
    ///
    /// SAFETY:
    /// - You must be passing in a pinned heap allocated reference, really this means you need a
    /// [std::pin::Pin]'ed box or arc.
    /// - As with all pointers the pointee must remain valid for the lifetime of this structure, it
    /// is on the user to ensure this.
    pub unsafe fn new(val: *const T) -> SendConst<T> {
        SendConst(val)
    }

    /// Return the inner pointer for use.
    pub fn to_ptr(&self) -> *const T {
        self.0
    }
}

// SAFETY: We only ever pass in pinned pointers, therefore there should be no reason not to be able
// to have a pointer be send in this case.
unsafe impl<T> Send for SendConst<T> {}

/// The [SendMut] construct is used to ensure we tell the compiler that we can indeed send these
/// pointers around threads. Now clearly this is not normally something that you want to do,
/// however we have a hard requirement for this to work. This is due to how [io_uring] works under
/// the hood when it comes to buffers, you must pass in pointers to mutable memory regions for
/// reads and immutable memory regions for sends. To do this we need to rely on pinned heap memory,
/// and as such these pointers are safe to pass around to other threads, primarily via the
/// executors themselves as they execute async tasks.
#[repr(transparent)]
pub struct SendMut<T>(*mut T);

impl<T> SendMut<T> {
    /// Create a new [SendConst] structure around a `*const T` of some kind, now there are some key
    /// safety considerations on using this function.
    ///
    /// SAFETY:
    /// - You must be passing in a pinned heap allocated reference, really this means you need a
    /// [std::pin::Pin]'ed box or arc.
    /// - As with all pointers the pointee must remain valid for the lifetime of this structure, it
    /// is on the user to ensure this.
    pub unsafe fn new(val: *mut T) -> SendMut<T> {
        SendMut(val)
    }

    /// Return the inner pointer for use.
    pub fn to_ptr(&self) -> *mut T {
        self.0
    }
}

// SAFETY: We only ever pass in pinned pointers, therefore there should be no reason not to be able
// to have a pointer be send in this case.
unsafe impl<T> Send for SendMut<T> {}
