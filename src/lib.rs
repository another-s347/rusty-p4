#![allow(warnings)]
#![feature(option_flattening)]
#![feature(async_await)]
#![feature(specialization)]
#![feature(const_generics)]

#[macro_use]
extern crate bitfield;
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate failure;
use crate::app::extended::{ExampleExtended, P4appBuilder};
use crate::app::Example;
use crate::context::Context;
use crate::event::CommonEvents;
use crate::p4rt::bmv2::Bmv2SwitchConnection;
use crate::proto::p4runtime::StreamMessageResponse;
use futures::future::Future;
use futures::sink::Sink;
use futures::stream::Stream;
use futures03::sink::SinkExt;
use futures03::stream::StreamExt;
use log::{debug, error, info, trace, warn};
use std::path::Path;

#[macro_use]
pub mod exported_macro;
pub mod app;
pub mod context;
pub mod error;
pub mod event;
pub mod p4rt;
pub mod proto;
pub mod representation;
pub mod restore;
pub mod util;

use crate::app::linkprobe::LinkProbeLoader;
use crate::p4rt::pipeconf::Pipeconf;
use crate::restore::Restore;
use std::collections::HashMap;
use tokio;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
