use async_trait::async_trait;
use std::net::Ipv4Addr;
use std::str::FromStr;
//use crate::app::async_app::ExampleAsyncApp;
//use crate::app::sync_app::AsyncWrap;
// use crate::core::DefaultContext;
use crate::event::{CommonEvents, Event, NorthboundRequest, PacketReceived};
use crate::proto::p4runtime::PacketIn;
use crate::util::flow::*;
use crate::util::packet::Ethernet;
use crate::util::packet::Packet;
use crate::util::value::EXACT;
use crate::util::value::{encode, LPM};
use bytes::{Bytes, BytesMut};
use futures::{FutureExt, future::{BoxFuture, Future}};
use log::{debug, error, info, trace, warn};
use std::any::{TypeId, Any};
use std::cell::{Ref, RefCell};
use std::ops::Deref;
use std::rc::Rc;
use std::{collections::HashMap, sync::{Arc, Mutex, MutexGuard}};
// use crate::core::context::Context;
use tuple_list::TupleList;
use tuple_list::tuple_list_type;

pub mod common;
pub mod graph;
pub mod device_manager;
// pub mod rest;
// pub mod statistic;
// pub mod raw_statistic;
// pub mod stratum_statistic;
// pub mod app_service;
pub mod options;
pub mod store;
pub mod default;

/// App is the core concept to build a controller, which is for custom logic and function. 
/// It can have dependency and option (also called configuration). 
/// Using dependency app can get instance of other apps when initializing.
/// So a app is required to be `clone + Send + Sync`, as it might be held by multiple apps and accessed from different threads.
#[async_trait]
pub trait App: Sync + Send + 'static + Sized {
    /// To support special function (like optional dependency), some generic container type (like `Arc` or `Option`) can also be defined as app.
    /// If you create a container type (`Option<T>`), use this field to specify the target type (`T`).
    /// For regular app, this field should be `Self`.
    type Container: App + Clone;
    /// Use this field to specify your dependency. 
    /// It should be a variadic tuple, but variadic tuple is not supported currently.
    /// So now, rusty-p4 use crate `tuple_list` to define dependencies.
    /// Use `()` for no dependency.
    type Dependency: Dependencies;
    type Option: options::AppOption;
    /// Name your app please.
    const Name: &'static str;

    /// This is where app get their dependencies, option then initialize, returns a new instance of app.
    /// Container type should not be `install`ed directly, so this method should not be called for container type. 
    fn init<S>(dependencies: Self::Dependency, store: &mut S, option: Self::Option) -> Self where S: store::AppStore;

    /// For container type, use this method to convert from target type to container type (`T -> Option<T>`).
    /// For regular type, simplely return will do.
    /// This method is called by rusty-p4 internally.
    fn from_inner(app: Option<Self::Container>) -> Option<Self>;

    async fn run(&self);
}

#[async_trait]
impl<T> App for Option<T> where T: App + Clone {
    type Container = T;
    type Dependency = T::Dependency;

    type Option = T::Option;
    const Name: &'static str = "Option";

    fn init<S>(dependencies: Self::Dependency, store: &mut S, option: Self::Option) -> Self where S: store::AppStore  {
        todo!()
    }

    fn from_inner(app: Option<Self::Container>) -> Option<Self> {
        Some(app)
    }

    async fn run(&self) {
        if let Some(s) = self {
            s.run().await;
        };
    }


}

#[async_trait]
impl<T> App for Arc<T> where T: App + Clone {
    type Container = T;
    type Dependency = T::Dependency;

    type Option = T::Option;
    const Name: &'static str = "Arc";

    fn init<S>(dependencies: Self::Dependency, store: &mut S, option: Self::Option) -> Self where S: store::AppStore  {
        todo!()
    }

    fn from_inner(app: Option<Self::Container>) -> Option<Self> {
        app.map(|x|Arc::new(x))
    }

    async fn run(&self) {
        self.as_ref().run().await;
    }
}

pub trait Dependencies:Sized {
    fn get<S>(store: &mut S) -> Option<Self> where S: store::AppStore;
}

impl<Head, Tail> Dependencies for (Head, Tail) where
    Head: App + Clone,
    Tail: Dependencies + TupleList + Clone,
{
    fn get<S>(store: &mut S) -> Option<Self> 
    where S: store::AppStore 
    {
        let a:Head = if let Some(a) = store.get::<Head>() {
            a
        } else {
            return None
        };

        let b:Tail = Tail::get(store)?;

        return Some((a,b));
    }
}

impl Dependencies for () {
    fn get<S>(store: &mut S) -> Option<Self> where S: store::AppStore {
        Some(())
    }
}

// #[async_trait]
// pub trait P4app<E, C>: 'static + Send
// where
//     E: Event,
//     C: Context<E>
// {
//     async fn on_start(self: &mut Self, ctx: &mut C) {}

//     async fn on_packet(
//         self: &mut Self,
//         packet: PacketReceived,
//         ctx: &mut C,
//     ) -> Option<PacketReceived> {
//         Some(packet)
//     }

//     async fn on_event(self: &mut Self, event: E, ctx: &mut C) -> Option<E> {
//         Some(event)
//     }

//     async fn on_request(self: &mut Self, request: NorthboundRequest, ctx: &mut C) {}

//     async fn on_context_update(self: &mut Self, ctx: &mut C) {}
// }

// pub struct Example {
//     pub counter: u32,
// }

// impl Example {
//     pub fn test(&self) {
//         println!("Example: counter={}", self.counter);
//     }
// }

// #[async_trait]
// impl<C> P4app<CommonEvents, C> for Example where C: Context<CommonEvents>{
//     async fn on_packet(
//         self: &mut Self,
//         packet: PacketReceived,
//         ctx: &mut C,
//     ) -> Option<PacketReceived> {
//         let parsed: Option<Ethernet<&[u8]>> = Ethernet::from_bytes(packet.packet.as_slice());
//         if let Some(ethernet) = parsed {
//             self.counter += 1;
//             info!(target:"Example App","Counter == {}, ethernet type == {:x}", self.counter, ethernet.ether_type);
//         } else {
//             warn!(target:"Example App","packet parse fail");
//         }
//         None
//     }

//     async fn on_event(
//         self: &mut Self,
//         event: CommonEvents,
//         ctx: &mut C,
//     ) -> Option<CommonEvents> {
//         match event {
//             CommonEvents::DeviceAdded(ref device) => {
//                 info!(target:"Example App","device up {:?}", device);
//                 //                let flow = flow! {
//                 //                    pipe:"MyIngress",
//                 //                    table:"ipv4_lpm" {
//                 //                        "hdr.ipv4.dstAddr"=>ipv4!(10.0.2.2)/32
//                 //                    }
//                 //                    action:"myTunnel_ingress"{
//                 //                        dst_id:100u32
//                 //                    }
//                 //                };
//                 //                ctx.insert_flow(flow, device.id);
//             }
//             _ => {}
//         }
//         None
//     }
// }
#[cfg(test)]
mod test {
    use std::sync::Arc;

    use tuple_list::tuple_list_type;

    use super::{App, store::{DefaultAppStore, install}};

    #[derive(Clone)]
    struct TestAppA;

    #[async_trait::async_trait]
    impl App for TestAppA {
        type Container = Self;

        type Dependency = ();

        type Option = ();

        const Name: &'static str = "TestAppA";

        fn init<S>(dependencies: Self::Dependency, store: &mut S, option: Self::Option) -> Self where S: super::store::AppStore {
            TestAppA
        }

        fn from_inner(app: Option<Self::Container>) -> Option<Self> {
            app
        }

        async fn run(&self) {
            todo!()
        }
    }

    #[derive(Clone)]
    struct TestAppB {
        test_app_a: TestAppA
    }

    #[async_trait::async_trait]
    impl App for TestAppB {
        type Container = Self;

        type Dependency = tuple_list_type!(TestAppA);

        type Option = ();

        const Name: &'static str = "TestAppB";

        fn init<S>(dependencies: Self::Dependency, store: &mut S, option: Self::Option) -> Self where S: super::store::AppStore {
            let tuple_list::tuple_list!(app_a) = dependencies;

            TestAppB {
                test_app_a: app_a
            }
        }

        fn from_inner(app: Option<Self::Container>) -> Option<Self> {
            todo!()
        }

        async fn run(&self) {
            todo!()
        }
    }

    #[derive(Clone)]
    struct TestAppC {
        test_app_a: Option<TestAppA>
    }

    #[async_trait::async_trait]
    impl App for TestAppC {
        type Container = Self;

        type Dependency = tuple_list_type!(Option<TestAppA>);

        type Option = ();

        const Name: &'static str = "TestAppC";

        fn init<S>(dependencies: Self::Dependency, store: &mut S, option: Self::Option) -> Self where S: super::store::AppStore {
            let tuple_list::tuple_list!(app_a) = dependencies;

            println!("have test_app_a:{}", app_a.is_some());

            TestAppC {
                test_app_a: app_a
            }
        }

        fn from_inner(app: Option<Self::Container>) -> Option<Self> {
            todo!()
        }

        async fn run(&self) {
            todo!()
        }
    }

    impl TestAppC {
        pub fn have_app_a(&self) -> bool {
            self.test_app_a.is_some()
        }
    }

    #[test]
    fn install_app() {
        let mut app_store = DefaultAppStore::default();
        let app_a:Arc<TestAppA> = install(&mut app_store, ()).unwrap();
    }

    #[test]
    fn install_app_dep() {
        let mut app_store = DefaultAppStore::default();
        let app_a:Arc<TestAppA> = install(&mut app_store, ()).unwrap();
        let app_b:Arc<TestAppB> = install(&mut app_store, ()).unwrap();
    }

    #[test]
    fn install_app_dep_failed() {
        let mut app_store = DefaultAppStore::default();
        assert_eq!(install::<_, TestAppB>(&mut app_store, ()).is_some(), false);
    }

    #[test]
    fn install_app_optional() {
        let mut app_store = DefaultAppStore::default();
        let app_a:Arc<TestAppA> = install(&mut app_store, ()).unwrap();
        let app_c:Arc<TestAppC> = install(&mut app_store, ()).unwrap();
        assert_eq!(app_c.have_app_a(), true);
    }

    #[test]
    fn install_app_optional_failed() {
        let mut app_store = DefaultAppStore::default();
        let app_c:Arc<TestAppC> = install(&mut app_store, ()).unwrap();
        assert_eq!(app_c.have_app_a(), false);
    }
}