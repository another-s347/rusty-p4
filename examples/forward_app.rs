use p4rt::{
    bmv2::{Bmv2ConnectionOption, Bmv2Event, Bmv2MasterUpdateOption},
    pipeconf::DefaultPipeconf,
};
use rusty_p4::app::{store::install, App};
use rusty_p4::event::PacketReceived;
use rusty_p4::flow;
use rusty_p4::p4rt;
use rusty_p4::representation::ConnectPoint;
use rusty_p4::util::async_trait;
use rusty_p4::util::publisher::Handler;
use rusty_p4::util::{tuple_list, tuple_list_type};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio;

#[derive(Clone)]
pub struct Forward {
    device_manager: rusty_p4::p4rt::bmv2::Bmv2Manager,
    bytes: Arc<AtomicUsize>,
}

#[async_trait]
impl App for Forward {
    type Container = Self;
    type Dependency = tuple_list_type!(rusty_p4::p4rt::bmv2::Bmv2Manager);

    type Option = ();

    fn init<S>(dependencies: Self::Dependency, _store: &mut S, _option: Self::Option) -> Self
    where
        S: rusty_p4::app::store::AppStore,
    {
        let tuple_list!(device_manager) = dependencies;
        let app = Forward {
            device_manager: device_manager.clone(),
            bytes: Default::default(),
        };
        device_manager.subscribe_packet(app.clone());
        device_manager.subscribe_event(app.clone());
        app
    }

    fn from_inner(app: Option<Self::Container>) -> Option<Self> {
        app
    }

    async fn run(&self) {}

    const Name: &'static str = "Forward";
}

#[async_trait]
impl Handler<PacketReceived> for Forward {
    async fn handle(&self, packet: PacketReceived) {
        let from = if let Some(from) = self.device_manager.get_packet_connectpoint(&packet) {
            from
        } else {
            panic!("???");
        };
        let b = packet.packet.len();
        let _bytes = self.bytes.fetch_add(b, Ordering::AcqRel);
        if from.port == 1 {
            self.device_manager
                .send_packet(
                    ConnectPoint {
                        device: from.device,
                        port: 2,
                    },
                    packet.packet,
                )
                .await
                .unwrap();
        } else if from.port == 2 {
            self.device_manager
                .send_packet(
                    ConnectPoint {
                        device: from.device,
                        port: 1,
                    },
                    packet.packet,
                )
                .await
                .unwrap();
        }
    }
}

#[async_trait]
impl Handler<Bmv2Event> for Forward {
    async fn handle(&self, event: Bmv2Event) {
        match event {
            Bmv2Event::DeviceAdded(device) => {
                let mut device = self.device_manager.get_device(device).unwrap();
                device
                    .insert_flow(flow! {
                        pipe: "MyIngress",
                        table: "acl" {
                            "hdr.ethernet.etherType" => 0x806 /*ARP*/,
                        },
                        action: "send_to_cpu" {},
                        priority: 1
                    })
                    .await
                    .unwrap();
                device
                    .insert_flow(flow! {
                        pipe: "MyIngress",
                        table: "acl" {
                            "hdr.ethernet.etherType" => 1u16 /*ICMP*/,
                        },
                        action: "send_to_cpu" {},
                        priority: 1
                    })
                    .await
                    .unwrap();
            }
        }
    }
}

#[tokio::main]
pub async fn main() {
    let pipeconf = DefaultPipeconf::new(
        "my_pipeconf",
        "./pipeconf/my_pipeconf.p4.p4info.bin",
        "./pipeconf/my_pipeconf.json",
    );

    let mut app_store = rusty_p4::app::store::DefaultAppStore::default();
    let device_manager: Arc<rusty_p4::p4rt::bmv2::Bmv2Manager> =
        install(&mut app_store, ()).unwrap();
    let forward: Arc<Forward> = install(&mut app_store, ()).unwrap();

    device_manager
        .add_device(
            "s1",
            "172.17.0.2:50001",
            Bmv2ConnectionOption {
                p4_device_id: 1,
                inner_device_id: Some(1),
                master_update: Some(Bmv2MasterUpdateOption {
                    election_id_high: 0,
                    election_id_low: 1,
                }),
            },
            pipeconf,
        )
        .await
        .unwrap();

    futures::future::join_all(vec![device_manager.run(), forward.run()]).await;
}
