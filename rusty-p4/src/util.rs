use crate::util::flow::Flow;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub mod flow;
pub mod packet;
pub mod value;
pub mod publisher;

pub fn hash<T>(obj: T) -> u64
where
    T: Hash,
{
    let mut hasher = DefaultHasher::new();
    obj.hash(&mut hasher);
    hasher.finish()
}
