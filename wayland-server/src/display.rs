use std::cell::RefCell;
use std::env;
use std::ffi::{OsStr, OsString};
use std::io::{Error as IoError, ErrorKind, Result as IoResult};
use std::os::unix::io::{IntoRawFd, RawFd};
use std::path::PathBuf;
use std::rc::Rc;

#[cfg(feature = "use_system_lib")]
use wayland_sys::server::wl_display;

use crate::imp::DisplayInner;

use crate::{Client, Global, Interface, Main, Resource};

/// The wayland display
///
/// This is the core of your wayland server, this object must
/// be kept alive as long as your server is running. It allows
/// you to manage listening sockets and clients.
pub struct Display {
    inner: Rc<RefCell<DisplayInner>>,
}

impl Display {
    /// Create a new display
    ///
    /// This method provides you a `Display` and inserts it into an existing
    /// `calloop::EventLoop`.
    ///
    /// Note that at this point, your server is not yet ready to receive connections,
    /// your need to add listening sockets using the `add_socket*` methods.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Display {
        Display {
            inner: DisplayInner::new(),
        }
    }

    /// Create a new global object
    ///
    /// This object will be advertised to all clients, and they will
    /// be able to instantiate it from their registries.
    ///
    /// Your implementation will be called whenever a client instantiates
    /// this global.
    ///
    /// The version specified is the **highest supported version**, you must
    /// be able to handle clients that choose to instantiate this global with
    /// a lower version number.
    pub fn create_global<I, F>(&mut self, version: u32, implementation: F) -> Global<I>
    where
        I: Interface + AsRef<Resource<I>> + From<Resource<I>>,
        F: FnMut(Main<I>, u32) + 'static,
    {
        assert!(
            version <= I::VERSION,
            "Cannot create global {} with version {}, maximum protocol version is {}.",
            I::NAME,
            version,
            I::VERSION
        );
        Global::create(
            self.inner
                .borrow_mut()
                .create_global(version, implementation, None::<fn(_) -> bool>),
        )
    }

    /// Create a new global object with a filter
    ///
    /// This object will be advertised to all clients, and they will
    /// be able to instantiate it from their registries.
    ///
    /// Your implementation will be called whenever a client instantiates
    /// this global.
    ///
    /// The version specified is the **highest supported version**, you must
    /// be able to handle clients that choose to instantiate this global with
    /// a lower version number.
    pub fn create_global_with_filter<I, F1, F2>(
        &mut self,
        version: u32,
        implementation: F1,
        mut filter: F2,
    ) -> Global<I>
    where
        I: Interface + AsRef<Resource<I>> + From<Resource<I>>,
        F1: FnMut(Main<I>, u32) + 'static,
        F2: FnMut(Client) -> bool + 'static,
    {
        assert!(
            version <= I::VERSION,
            "Cannot create global {} with version {}, maximum protocol version is {}.",
            I::NAME,
            version,
            I::VERSION
        );
        Global::create(self.inner.borrow_mut().create_global(
            version,
            implementation,
            Some(move |client_inner| filter(Client::make(client_inner))),
        ))
    }

    /// Flush events to the clients
    ///
    /// Will send as many pending events as possible to the respective sockets of the clients.
    /// Will not block, but might not send everything if the socket buffer fills up.
    pub fn flush_clients(&mut self) {
        self.inner.borrow_mut().flush_clients()
    }

    /// Dispatches all pending messages to their respective filters
    ///
    /// This method will block waiting for messages until one of these occur:
    ///
    /// - Some messages are received, in which case all pending messages are processed
    /// - The timeout is reached
    /// - An error occurs
    ///
    /// If `timeout` is a duration of 0, this function will only process pending messages and then
    /// return, not blocking.
    ///
    /// In general for good performance you will want to integrate the `Display` into your own event loop,
    /// monitoring the file descriptor retrieved by the `get_poll_fd()` method, and only calling this method
    /// when messages are available, with a timeout of `0`.
    pub fn dispatch(&mut self, timeout: std::time::Duration) -> IoResult<()> {
        let ms = timeout.as_millis();
        let clamped_timeout = if ms > std::i32::MAX as u128 {
            std::i32::MAX
        } else {
            ms as i32
        };
        self.inner.borrow_mut().dispatch(clamped_timeout)
    }

    /// Retrieve the underlying file descriptor
    ///
    /// This file descriptor can be monitored for activity with a poll/epoll like mechanism.
    /// When it becomes readable, this means that there are pending messages that would be dispatched if
    /// you call `dispatch` with a timeout of 0.
    ///
    /// You should not use this file descriptor for any other purpose than monitoring it.
    pub fn get_poll_fd(&self) -> RawFd {
        self.inner.borrow().get_poll_fd()
    }
}

impl Display {
    /// Add a listening socket to this display
    ///
    /// Wayland clients will be able to connect to your compositor from this socket.
    ///
    /// Socket will be created in the directory specified by the environment variable
    /// `XDG_RUNTIME_DIR`.
    ///
    /// If a name is provided, it is used. Otherwise, if `WAYLAND_DISPLAY` environment
    /// variable is set, its contents are used as socket name. Otherwise, `wayland-0` is used.
    ///
    /// Errors if `name` contains an interior null, or if `XDG_RUNTIME_DIR` is not set,
    /// or if specified could not be bound (either it is already used or the compositor
    /// does not have the rights to create it).
    pub fn add_socket<S>(&mut self, name: Option<S>) -> IoResult<()>
    where
        S: AsRef<OsStr>,
    {
        self.inner.borrow_mut().add_socket(name)
    }

    /// Add an automatically named listening socket to this display
    ///
    /// Wayland clients will be able to connect to your compositor from this socket.
    ///
    /// Socket will be created in the directory specified by the environment variable
    /// `XDG_RUNTIME_DIR`. The directory is scanned for any name in the form `wayland-$d` with
    /// `0 <= $d < 32` and the first available one is used.
    ///
    /// Errors if `XDG_RUNTIME_DIR` is not set, or all 32 names are already in use.
    pub fn add_socket_auto(&mut self) -> IoResult<OsString> {
        self.inner.borrow_mut().add_socket_auto()
    }

    /// Add existing listening socket to this display
    ///
    /// Wayland clients will be able to connect to your compositor from this socket.
    ///
    /// The existing socket fd must already be created, opened, and locked.
    /// The fd must be properly set to CLOEXEC and bound to a socket file
    /// with both bind() and listen() already called. An error is returned
    /// otherwise.
    pub fn add_socket_from<T>(&mut self, socket: T) -> IoResult<()>
    where
        T: IntoRawFd,
    {
        unsafe { self.add_socket_fd(socket.into_raw_fd()) }
    }

    /// Add existing listening socket to this display from a raw file descriptor
    ///
    /// Wayland clients will be able to connect to your compositor from this socket.
    ///
    /// The library takes ownership of the provided socket if this method returns
    /// successfully.
    ///
    /// The existing socket fd must already be created, opened, and locked.
    /// The fd must be properly set to CLOEXEC and bound to a socket file
    /// with both bind() and listen() already called. An error is returned
    /// otherwise.
    pub unsafe fn add_socket_fd(&self, fd: RawFd) -> IoResult<()> {
        self.inner.borrow_mut().add_socket_fd(fd)
    }

    /// Create a new client to this display from an already-existing connected Fd
    pub unsafe fn create_client(&self, fd: RawFd) -> Client {
        Client::make(self.inner.borrow_mut().create_client(fd))
    }
}

#[cfg(feature = "use_system_lib")]
impl Display {
    /// Retrieve a pointer from the C lib to this `wl_display`
    pub fn c_ptr(&self) -> *mut wl_display {
        self.inner.borrow().ptr()
    }
}

pub(crate) fn get_runtime_dir() -> IoResult<PathBuf> {
    match env::var_os("XDG_RUNTIME_DIR") {
        Some(s) => Ok(s.into()),
        None => Err(IoError::new(
            ErrorKind::NotFound,
            "XDG_RUNTIME_DIR env variable is not set",
        )),
    }
}
