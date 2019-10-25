#![allow(warnings)]
#![feature(option_flattening)]
#![feature(linked_list_extras)]
#![feature(specialization)]
#![recursion_limit = "512"]
#[macro_use]
extern crate failure;

use proc_macro_hack::proc_macro_hack;

#[proc_macro_hack(support_nested)]
pub use macro_impl::flow;
#[proc_macro_hack(support_nested)]
pub use macro_impl::flow_match;
//#[macro_use]
//pub mod exported_macro;
pub mod app;
pub mod context;
pub mod entity;
pub mod error;
pub mod event;
pub mod p4rt;
pub mod proto;
pub mod representation;
pub mod service;
pub mod util;
