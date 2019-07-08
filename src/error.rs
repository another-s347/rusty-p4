use std::error::Error as StdError;
use std::result::Result as StdResult;
use std::fmt::Formatter;

pub type Error = Box<dyn StdError>;
pub type Result<T> = StdResult<T, Error>;

#[derive(Debug)]
pub enum ContextError {
    DeviceNotConnected(String)
}

impl std::fmt::Display for ContextError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        unimplemented!()
    }
}

impl std::error::Error for ContextError {

}