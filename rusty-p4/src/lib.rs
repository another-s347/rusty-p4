#![allow(warnings)]
#![feature(option_flattening)]
#![feature(linked_list_extras)]
#![feature(specialization)]
#![feature(bind_by_move_pattern_guards)]
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate failure;

#[macro_use]
pub mod exported_macro;
pub mod app;
pub mod context;
pub mod entity;
pub mod error;
pub mod event;
pub mod p4rt;
pub mod proto;
pub mod representation;
pub mod restore;
pub mod service;
pub mod util;
