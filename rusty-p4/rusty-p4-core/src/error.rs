use crate::representation::DeviceID;
use std::error::Error as StdError;
use std::fmt::{Display, Formatter};
use std::result::Result as StdResult;
use thiserror::Error;
#[derive(Error, Debug)]
pub enum DeviceError {
    #[error("Device {:?} not connected", device)]
    DeviceNotConnected {
        device: DeviceID
    },
    #[error("Device {:?} gRPC Error {:?}", device, error)]
    DeviceGrpcError {
        device: DeviceID,
        error: tonic::Status
    },
    #[error("Device config file {} error: {:?}", path, error)]
    DeviceConfigFileError {
        path: String,
        error: std::io::Error
    }
}

#[derive(Error, Debug)]
pub enum InternalError {

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
    #[error("Internal error {:#?}", 0)]
    Internal(#[from] InternalError),
    #[error(transparent)]
    App(#[from] AppError),
    #[error(transparent)]
    Service(#[from] ServiceError),
    #[error(transparent)]
    Device(#[from] DeviceError)
}

pub type Result<T> = std::result::Result<T, MyError>;