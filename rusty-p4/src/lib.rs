#![allow(warnings)]
#![recursion_limit = "512"]

use proc_macro_hack::proc_macro_hack;

#[proc_macro_hack(support_nested)]
pub use macro_impl::flow;
#[proc_macro_hack(support_nested)]
pub use macro_impl::flow_match;
//#[macro_use]
//pub mod exported_macro;
pub mod app;
// pub mod core;
pub mod entity;
pub mod error;
pub mod event;
pub mod p4rt;
pub mod proto;
pub mod representation;
pub mod service;
pub mod util;
pub mod gnmi;
pub mod system;
