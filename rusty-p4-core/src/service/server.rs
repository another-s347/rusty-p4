use serde::Serialize;

pub trait Server {
    type EncodeTarget;
    const NAME: &'static str;

    fn encode<T>(response: T) -> Self::EncodeTarget
    where
        T: Serialize;
}
