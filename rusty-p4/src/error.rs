use crate::representation::DeviceID;
use failure::{Backtrace, Context, Fail};
use std::error::Error as StdError;
use std::fmt::{Display, Formatter};
use std::result::Result as StdResult;

#[derive(Debug)]
pub struct ContextError {
    inner: Context<ContextErrorKind>,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum ContextErrorKind {
    #[fail(display = "Device not connected: {:?}.", device)]
    DeviceNotConnected { device: DeviceID },
    #[fail(display = "Internal connection error.")]
    ConnectionError,
    #[fail(display = "Entity cannot be converted to proto.")]
    EntityIsNone,
}

impl Fail for ContextError {
    fn cause(&self) -> Option<&Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl Display for ContextError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

impl From<ContextErrorKind> for ContextError {
    fn from(kind: ContextErrorKind) -> ContextError {
        ContextError {
            inner: Context::new(kind),
        }
    }
}

impl From<Context<ContextErrorKind>> for ContextError {
    fn from(inner: Context<ContextErrorKind>) -> ContextError {
        ContextError { inner }
    }
}

#[derive(Debug)]
pub struct ConnectionError {
    inner: Context<ConnectionErrorKind>,
}

#[derive(Clone, Eq, PartialEq, Debug, Fail)]
pub enum ConnectionErrorKind {
    #[fail(display = "Sending gRPC request failed.")]
    GRPCSendError,
    #[fail(display = "Error when processing device config file.")]
    DeviceConfigFileError,
    #[fail(display = "Build request from pipeconf failed: {}.", _0)]
    PipeconfError(String),
}

impl Fail for ConnectionError {
    fn cause(&self) -> Option<&Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl Display for ConnectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

impl From<ConnectionErrorKind> for ConnectionError {
    fn from(kind: ConnectionErrorKind) -> ConnectionError {
        ConnectionError {
            inner: Context::new(kind),
        }
    }
}

impl From<Context<ConnectionErrorKind>> for ConnectionError {
    fn from(inner: Context<ConnectionErrorKind>) -> ConnectionError {
        ConnectionError { inner }
    }
}
