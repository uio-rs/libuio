use io_uring::{cqueue, squeue};

/// A [CompletionStatus] represents the result of resolving a given completion passed to the
/// [super::UringDriver] executing it. The completion is responsible for informing  the ring
/// what the given state of the complation:
///
/// - [CompletionStatus::Armed] ready to go and no further action is needed.
/// - [CompletionStatus::Rearm] a multi-shot event needs to be re-armed.
/// - [CompletionStatus::Finalized] the completion is done and can be dropped.
///
/// See [Completion] for more details on the use cases for each option, and when to use which
/// option for your use case.
pub enum CompletionStatus {
    /// Armed completions are for multi-shot events that are already armed, and haven't been
    /// invalidated due to an error or like wise event.
    Armed,
    /// Rearmed completions are for multi-shot events that ran into some error or like wise event
    /// and need to be resubmitted for further events to be tracked.
    ///
    /// Note that the state index in this case is re-used and so is the rest of the current state.
    Rearm,
    /// A completed and finalized completion event, this is no longer usable and should be
    /// forgotten, it is an error to reuse a finalized completion.
    Finalized,
}

/// A [Completion] represents a [super::UringDriver] I/O event and its ultimate resolution once the
/// event has been completed by the driver. This is used to hand back the result of the async I/O
/// operation back to the calling future. This is done through the [Completion::resolve] method
/// which takes in the [cqueue::Entry] completion event from the underlying [io_uring] and handles
/// the result of the event. This then returns to the [super::UringDriver] instructing on what to
/// do with the remaining state data. This trait is also used be the [super::UringDriver] to
/// generate the [squeue::Entry] for submission into the underlying [io_uring] on initial creation
/// and when instructed to do so with a [CompletionStatus::Rearm] response from
/// [Completion::resolve].
///
/// # Examples
///
/// To see a complete example of a multi-shot completion see: [crate::net::Incoming]
///
/// An example of implementing a one-shot completion is:
///
/// ```no_run
/// use io_uring::{opcode, types::CancelBuilder};
///
/// use libuio::io_uring::{Completion, CompletionStatus};
///
/// /// A simple cancel event based on a state index.
/// pub struct Cancel {
///     index: usize,
/// }
///
/// impl Cancel {
///     /// Create a new [Cancel] event targeting the given state index.
///     pub fn new(index: usize) -> Cancel {
///         Cancel { index }
///     }
/// }
///
/// impl Completion for Cancel {
///     fn resolve(&mut self, _: io_uring::cqueue::Entry) -> CompletionStatus {
///         // We don't have anything to do, and we are a 'one shot' event so we need to return
///         // 'Finalized'
///         CompletionStatus::Finalized
///     }
///
///     fn as_entry(&mut self) -> io_uring::squeue::Entry {
///         // We are targeting a state index which in our cases is "user_data" so lets target that
///         // with the CancelBuilder and return a valid opcode.
///         let cancel = CancelBuilder::user_data(self.index as u64).all();
///         opcode::AsyncCancel2::new(cancel).build()
///     }
/// }
/// ```
pub trait Completion: Send {
    /// Handle resolving the completion once this is called by the [super::UringDriver]. This
    /// method is responsible for passing the result of the event back to the future that created
    /// this completion.
    ///
    /// The response of this method informs the caller, what to do with the completion after the
    /// call to resolve. There are three options to pick from via the [CompletionStatus] enum:
    ///
    /// ## [CompletionStatus::Armed]
    ///
    /// Use this return for multi-shot events, that have a truthy response from [cqueue::more] on
    /// the flags returned from [cqueue::Entry::flags]. This will instruct the [super::UringDriver]
    /// to do nothing and just continue on leaving this completion to be re-used later on by a
    /// subsequent event from the kernel.
    ///
    /// ## [CompletionStatus::Rearm]
    ///
    /// Use this return for multi-shot events, that have a falsy response from [cqueue::more] on
    /// the flags return from [cqueue::Entry::falgs]. This will instruct the [super::UringDriver]
    /// to re-submit this completion by calling [Completion::as_entry] and re-submitting it to the
    /// ring.
    ///
    /// ## [CompletionStatus::Finalized]
    ///
    /// Use this return for all one-shot events and for any errored multi-shot events. This will
    /// instruct the [super::UringDriver] to remove the state for this completion and drop its
    /// references to it.
    ///
    ///
    /// # Errors
    ///
    /// The return value here can't have errors as the caller, a [super::UringDriver], has no
    /// ability to handle any errors while handling the response. The only party that could is the
    /// caller of the future of this completion, so any errors must be passed back via the future.
    ///
    /// # Panics
    ///
    /// This method should never panic, any panic's here will cause the entire event loop to fail.
    /// So the only acceptable reason to panic is a completely irrcoverable error. There are few of
    /// these at this layer so be forewarned.
    fn resolve(&mut self, value: cqueue::Entry) -> CompletionStatus;

    /// Handle creating a [squeue::Entry] suitable for this completion, this is generally done via
    /// the [io_uring::opcode] module and creating whatever operation is needed. The caller, a
    /// [super::UringDriver] will update the [squeue::Entry::user_data] call *after* the call to
    /// [Completion::as_entry] so there is no need to set the user_data in this call.
    ///
    /// # Panics
    ///
    /// This method should never panic, any panic's here will cause the entire event loop to fail.
    /// So the only acceptable reason to panic is a completely irrecoverable error. There are few
    /// of these at this layer so be forewarned.
    fn as_entry(&mut self) -> squeue::Entry;
}
