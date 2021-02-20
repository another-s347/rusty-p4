use crate::representation::DeviceID;
use std::error::Error as StdError;
use std::fmt::{Display, Formatter};
use std::result::Result as StdResult;
use thiserror::Error;
#[derive(Error, Debug)]
pub enum DeviceError {
    #[error("Device {:?} not connected", device)]
    DeviceNotConnected { device: DeviceID },
    #[error("Device {:?} gRPC Error {:?}", device, error)]
    DeviceGrpcError {
        device: DeviceID,
        error: tonic::Status,
    },
    #[error("Device {:?} gRPC transport Error {:?}", device, error)]
    DeviceGrpcTransportError {
        device: DeviceID,
        error: tonic::transport::Error,
    },
    #[error("Device config file {} error: {:?}", path, error)]
    DeviceConfigFileError { path: String, error: std::io::Error },
    #[error("Master not acquired, {:?}", reason)]
    NotMaster { device: DeviceID, reason: String },
    #[error("Device {:?} error: {}", device, error)]
    Other { device: DeviceID, error: String },
}

#[derive(Error, Debug)]
pub enum InternalError {
    #[error("Device not found, which should not happen.")]
    DeviceNotFound,
    #[error("{}", err)]
    Other { err: String },
}

#[derive(Error, Debug)]
pub enum ServiceError {
    #[error("Service {} not found.", 0)]
    ServiceNotFound(String),
    #[error("Action {} not found.", 0)]
    ActionNotFound(String),
    #[error("Process request error")]
    RequestError(#[source] anyhow::Error),
}

#[derive(Error, Debug)]
pub enum AppError {
    #[error("App {} not found.", 0)]
    AppNotFound(String),
}

#[derive(Error, Debug)]
pub enum MyError {
    #[error(
        "This is a internal error, if you see this, please report to developer: {:#?}, ",
        0
    )]
    Internal(#[from] InternalError),
    #[error(transparent)]
    App(#[from] AppError),
    #[error(transparent)]
    Service(#[from] ServiceError),
    #[error(transparent)]
    Device(#[from] DeviceError),
}

pub type Result<T> = std::result::Result<T, MyError>;
