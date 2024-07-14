use std::{
    cmp::Ordering,
    collections::VecDeque,
    os::fd::{FromRawFd, OwnedFd, RawFd},
    pin::Pin,
    ptr,
    sync::mpsc::{self, Receiver},
    task::Waker,
};

use io_uring::{
    cqueue, opcode, squeue,
    types::{self, CancelBuilder, SubmitArgs, Timespec},
    IoUring,
};
use nix::libc;
use slab::Slab;

use crate::net::SocketAddrC;

use super::{AsyncResult, Error, Result, Token};

/// The [Uring] represents the meat of the event loop, and handles two key pieces of the puzzle,
/// first off it handles registration of various [io_uring::opcode]'s that enable waiting for
/// different types of I/O completion events. The it exposes the actual event loop itself, for
/// execution in the executor. The idea is that the [Uring::run] call is executed once per loop by
/// each worker running. This will produce N completion events and generally wake up multiple
/// tasks. The executor then pulls all available tasks polling them each and then repeating the
/// process.
pub struct Uring {
    ring: IoUring,
    backlog: VecDeque<squeue::Entry>,
    tokens: Slab<Token>,
}

impl Uring {
    /// Create a new [Uring] with the specified number of entries.
    pub(crate) fn new(entries: u32) -> Result<Uring> {
        let ring = IoUring::builder()
            // .setup_defer_taskrun()
            // .setup_single_issuer()
            .build(entries)?;

        let backlog = VecDeque::with_capacity(1024);
        let tokens = Slab::with_capacity(1024);

        Ok(Uring {
            ring,
            backlog,
            tokens,
        })
    }

    /// Register a multi shot accept event, meaning this event will continue to fire repeatedly.
    pub(crate) fn register_incoming(&mut self, fd: RawFd) -> (Receiver<Result<OwnedFd>>, usize) {
        let (sender, receiver) = mpsc::channel();

        let token = Token::Incoming {
            sender,
            waker: None,
        };
        let token_idx = self.tokens.insert(token);

        let entry = opcode::AcceptMulti::new(types::Fd(fd))
            .build()
            .user_data(token_idx as _);
        self.push(entry);

        (receiver, token_idx)
    }

    /// Register a one shot accept event in the loop.
    pub(crate) fn register_accept(&mut self, fd: RawFd) -> (AsyncResult<Result<OwnedFd>>, usize) {
        let result = AsyncResult::new();
        let token = Token::Accept {
            result: result.clone(),
            waker: None,
        };
        let token_idx = self.tokens.insert(token);

        let entry = opcode::Accept::new(types::Fd(fd), ptr::null_mut(), ptr::null_mut())
            .build()
            .user_data(token_idx as _);
        self.push(entry);

        (result, token_idx)
    }

    /// Register a one shot receive event in the loop.
    pub(crate) fn register_recv(
        &mut self,
        fd: RawFd,
        buf: &mut [u8],
    ) -> (AsyncResult<Result<(usize, bool)>>, usize) {
        let result = AsyncResult::new();
        let token = Token::Recv {
            result: result.clone(),
            waker: None,
        };
        let token_idx = self.tokens.insert(token);

        let entry = opcode::Recv::new(types::Fd(fd), buf.as_mut_ptr(), buf.len() as u32)
            .build()
            .user_data(token_idx as _);
        self.push(entry);

        (result, token_idx)
    }

    /// Register a one shot send event in the loop.
    pub(crate) fn register_send(
        &mut self,
        fd: RawFd,
        buf: &[u8],
    ) -> (AsyncResult<Result<usize>>, usize) {
        let result = AsyncResult::new();
        let token = Token::Send {
            result: result.clone(),
            waker: None,
        };
        let token_idx = self.tokens.insert(token);

        let entry = opcode::Send::new(types::Fd(fd), buf.as_ptr(), buf.len() as u32)
            .build()
            .user_data(token_idx as _);
        self.push(entry);

        (result, token_idx)
    }

    /// Register a one shot connect event in the loop.
    pub(crate) fn register_connect(
        &mut self,
        fd: RawFd,
        addr: &Pin<Box<SocketAddrC>>,
        addr_len: libc::socklen_t,
    ) -> (AsyncResult<Result<()>>, usize) {
        let result = AsyncResult::new();
        let token = Token::Connect {
            result: result.clone(),
            waker: None,
        };
        let token_idx = self.tokens.insert(token);

        let entry = opcode::Connect::new(types::Fd(fd), addr.as_ptr(), addr_len)
            .build()
            .user_data(token_idx as _);
        self.push(entry);

        (result, token_idx)
    }

    /// Do a best effort cancel of this token, its best effort in that we have no idea at this
    /// point if the given event has already fired and we just haven't gotten to the completion
    /// event yet.
    pub(crate) fn deregister(&mut self, token: usize) {
        // Drop the token from our slab.
        if self.tokens.try_remove(token).is_none() {
            // If none exist go ahead and ignore this.
            return;
        }

        // Setup a cancel for all events associated with this token.
        let cancel_token = Token::Cancel;
        let token_idx = self.tokens.insert(cancel_token);

        let cancel = CancelBuilder::user_data(token as u64).all();
        let entry = opcode::AsyncCancel2::new(cancel)
            .build()
            .user_data(token_idx as _);

        // Push the cancel onto the submission queue.
        self.push(entry);
    }

    /// Override the waker on the internal event state. This ensures that if a task is moved
    /// between threads we get the proper waker on completion of the event.
    pub(crate) fn set_waker(&mut self, user_data: usize, waker: Waker) {
        if let Some(token) = self.tokens.get_mut(user_data) {
            token.set_waker(waker);
        }
    }

    fn clear_backlog(&mut self) -> Result<()> {
        let (submitter, mut sq, _) = self.ring.split();
        loop {
            if sq.is_full() {
                match submitter.submit() {
                    Ok(_) => (),
                    Err(ref err) if err.raw_os_error() == Some(libc::EBUSY) => break,
                    Err(err) => return Err(err.into()),
                }
            }
            sq.sync();

            match self.backlog.pop_front() {
                Some(sqe) => unsafe {
                    let _ = sq.push(&sqe);
                },
                None => break,
            }
        }
        Ok(())
    }

    fn push(&mut self, entry: squeue::Entry) {
        // Push the new entry onto the submission queue, or fallback to our local VecDeque on
        // error. The error in question here, is a queue full error, and is meant to be retried,
        // which is handled in the clear_backlog() fn above.
        unsafe {
            if self.ring.submission().push(&entry).is_err() {
                self.backlog.push_back(entry);
            }
        }
    }

    /// The main event loop run call, this should be executed once per loop by the worker threads.
    /// It will execute for up to 100ms before bailing waiting for I/O events. This is so that we
    /// don't starve non-IO related tasks and can still push forward on other tasks while we get
    /// more completions. In practice this should be completely transparent.
    ///
    /// Generally the flow is this:
    /// - Start by calling `submit_with_args`.
    /// - Then iterate over all available completion events, waking tasks as needed.
    /// - Returning to let the executor poll the woken tasks.
    pub(crate) fn run(&mut self) -> Result<()> {
        let spec = Timespec::new().nsec(100_000_000); // wait up to 100ms
        let args = SubmitArgs::new().timespec(&spec);

        match self.ring.submitter().submit_with_args(1, &args) {
            // We got at least one event lets process it.
            Ok(_) => {}
            Err(e) => match e.raw_os_error() {
                Some(libc::EBUSY) => {} // Our ring is busy, do nothing and just continue on.
                Some(libc::ETIME) => {} // We timed out on our ring wait, do nothing and just continue on.
                _ => return Err(Error::from(e)), // Got an unexpected error bail out.
            },
        }

        self.clear_backlog()?;

        let (_, _, mut cq) = self.ring.split();
        for cqe in &mut cq {
            let ret = cqe.result();
            let user_data = cqe.user_data();

            let token = match self.tokens.get_mut(user_data as usize) {
                Some(token) => token,
                None => {
                    continue;
                }
            };

            match token {
                Token::Cancel => {
                    self.tokens.remove(user_data as usize);
                }
                Token::Accept { result, waker } => {
                    let res = if ret < 0 {
                        Err((-ret).into())
                    } else {
                        Ok(unsafe { OwnedFd::from_raw_fd(ret) })
                    };
                    result.set(res);

                    if let Some(waker) = waker.take() {
                        waker.wake();
                    }

                    // Since we are performing a single Accept we need to remove this token to
                    // release back into the system.
                    self.tokens.remove(user_data as usize);
                }
                Token::Incoming { sender, waker } => {
                    if ret < 0 {
                        sender.send(Err((-ret).into()))?;
                    } else {
                        // SAFETY: We know for sure nothing else can possibly own this FD yet, so
                        // we can safely wrap it here and only here in an OwnedFd.
                        sender.send(Ok(unsafe { OwnedFd::from_raw_fd(ret) }))?;
                    }

                    if let Some(waker) = waker.take() {
                        waker.wake();
                    }
                }
                Token::Recv { result, waker } => {
                    let res = match ret.cmp(&0) {
                        Ordering::Less => Err((-ret).into()),
                        Ordering::Equal => Err(Error::Disconnected),
                        Ordering::Greater => Ok((ret as usize, cqueue::sock_nonempty(cqe.flags()))),
                    };
                    result.set(res);

                    if let Some(waker) = waker.take() {
                        waker.wake();
                    }

                    self.tokens.remove(user_data as usize);
                }
                Token::Send { result, waker } => {
                    let res = match ret.cmp(&0) {
                        Ordering::Less => Err((-ret).into()),
                        Ordering::Equal => Err(Error::Disconnected),
                        Ordering::Greater => Ok(ret as usize),
                    };
                    result.set(res);

                    if let Some(waker) = waker.take() {
                        waker.wake();
                    }

                    self.tokens.remove(user_data as usize);
                }
                Token::Connect { result, waker, .. } => {
                    let res = match ret.cmp(&0) {
                        Ordering::Less => Err((-ret).into()),
                        _ => Ok(()),
                    };
                    result.set(res);

                    if let Some(waker) = waker.take() {
                        waker.wake();
                    }

                    self.tokens.remove(user_data as usize);
                }
            }
        }
        Ok(())
    }
}
