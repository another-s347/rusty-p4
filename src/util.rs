use crate::util::flow::Flow;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub mod flow;
pub mod packet;
pub mod value;

pub fn hash_flow(obj: &Flow) -> u64 {
    let mut hasher = DefaultHasher::new();
    obj.hash(&mut hasher);
    hasher.finish()
}
