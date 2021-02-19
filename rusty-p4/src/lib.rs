#![allow(warnings)]
#![recursion_limit = "512"]

/// A macro to help creating flow entry.
/// 
/// # Examples
/// ```
/// flow! {
///     pipe: "your pipe name",
///     table: "your table name" {
///         "match key 1" => 1u32,       // exact match
///         "match key 2" => 1u32..2u32, // range match
///         "match key 3" => 1u32/8,     // lpm match
///         "match key 4" => 1u32&2u32   // ternary match
///     },
///     action: "your action name" {
///         "param name 1": 1,
///         "param name 2": 2,
///     },
///     priority: 1
/// }
/// ```
/// will generate a [util::flow::Flow] struct.
/// ### Merge matches.
/// You can reuse your flow match struct by:
/// ```
/// let other_matches = flow_match!{...};
/// flow! {
///     pipe: "your pipe name",
///     table: "your table name" {
///         "match key 1" => 1u32,       // exact match
///         "match key 2" => 1u32..2u32, // range match
///         "match key 3" => 1u32/8,     // lpm match
///         "match key 4" => 1u32&2u32,   // ternary match
///         ..other_matches
///     },
///     action: "your action name" {
///         "param name 1": 1,
///         "param name 2": 2,
///     },
///     priority: 1
/// }
/// ```
/// which will call [util::flow::FlowTable::merge_matches] to merge `other_matches` to this flow.
pub use macro_impl::flow;
/// A macro to help creating flow match entry.
/// 
/// # Examples
/// ```
/// flow_match! {
///     "match key 1" => 1u32,       // exact match
///     "match key 2" => 1u32..2u32, // range match
///     "match key 3" => 1u32/8,     // lpm match
///     "match key 4" => 1u32&2u32   // ternary match
/// }
/// ```
/// will generate a [util::flow::FlowMatches] struct.
pub use macro_impl::flow_match;
//#[macro_use]
//pub mod exported_macro;
// pub mod app;
// // pub mod core;
// pub mod entity;
// pub mod error;
// pub mod event;
// pub mod p4rt;
// pub mod proto;
// pub mod representation;
// pub mod service;
// pub mod util;
// pub mod gnmi;
// pub mod system;
pub use rusty_p4_core::util;
pub use rusty_p4_core::app;
pub use rusty_p4_core::p4rt;
pub use rusty_p4_core::representation;
pub use rusty_p4_packet::packet;
pub use rusty_p4_core::event;
pub use rusty_p4_core::service;