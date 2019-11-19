use std::collections::{HashMap, HashSet};
use std::convert::{TryFrom, TryInto};
use std::fmt::Debug;
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};

use byteorder::BigEndian;
use byteorder::ByteOrder;
use bytes::Bytes;
use failure::ResultExt;
use futures::future::FutureExt;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use log::{debug, error, info, trace, warn};
use tokio::runtime::current_thread::Handle;
use tokio::runtime::Runtime;

use crate::app::P4app;
use crate::core::driver::ContextDriver;
use crate::entity::UpdateType;
use crate::error::{ContextError, ContextErrorKind};
use crate::event::{
    CommonEvents, CoreEvent, CoreRequest, Event, NorthboundRequest, PacketReceived,
};
use crate::p4rt::bmv2::{Bmv2SwitchConnection, Bmv2MasterUpdateOption, Bmv2ConnectionOption};
use crate::p4rt::pipeconf::{Pipeconf, PipeconfID};
use crate::p4rt::pure::{new_packet_out_request, new_set_entity_request, new_write_table_entry};
use crate::proto::p4runtime::{
    stream_message_response, Entity, Index, MeterEntry, PacketIn, StreamMessageRequest,
    StreamMessageResponse, Uint128, Update, WriteRequest, WriteResponse,
};
use rusty_p4_proto::proto::v1::{
    MasterArbitrationUpdate
};
use crate::representation::{ConnectPoint, Device, DeviceID, DeviceType};
use crate::util::flow::Flow;
use crate::core::Context;
use crate::core::connection::bmv2::Bmv2Connection;
use crate::core::connection::{ConnectionBox, Connection};
use crate::core::connection::stratum_bmv2::StratumBmv2Connection;
use crate::p4rt::stratum_bmv2::{StratumBmv2SwitchConnection, StratumBmv2ConnectionOption};
use futures::channel::mpsc::{Sender, Receiver};

type P4RuntimeClient =
crate::proto::p4runtime::client::P4RuntimeClient<tonic::transport::channel::Channel>;

#[derive(Copy, Clone, Default)]
pub struct ContextConfig {
    pub enable_netconfiguration: bool,
}

pub struct Core<E> {
    pub(crate) pipeconf: Arc<HashMap<PipeconfID, Pipeconf>>,
    pub(crate) core_channel_sender: Sender<CoreRequest>,
    pub(crate) event_sender: Sender<CoreEvent<E>>,
    pub(crate) connections: HashMap<DeviceID, ConnectionBox>,
    pub(crate) config: ContextConfig,
}

impl<E> Core<E>
    where
        E: Event + Clone + 'static + Send,
{
    pub async fn try_new<T>(
        pipeconf: HashMap<PipeconfID, Pipeconf>,
        mut app: T,
        config: ContextConfig,
        northbound_channel: Option<Receiver<NorthboundRequest>>,
    ) -> Result<(Context<E>, ContextDriver<E, T>), ContextError>
        where
            T: P4app<E> + 'static,
    {
        let (app_s, app_r) = futures::channel::mpsc::channel(10240);

        let (s, mut r) = futures::channel::mpsc::channel(10240);

        let mut obj = Core {
            pipeconf: Arc::new(pipeconf),
            core_channel_sender: s,
            event_sender: app_s,
            connections:HashMap::new(),
            config,
        };
        let mut context_handle = obj.get_handle();

        app.on_start(&mut context_handle).await;

        let northbound_channel = if let Some(r) = northbound_channel {
            r
        } else {
            let (w, r) = futures::channel::mpsc::channel(10240);
            r
        };

        let driver = ContextDriver {
            core_request_receiver: r,
            event_receiver: app_r,
            request_receiver: northbound_channel,
            app,
            ctx: obj,
        };

        Ok((context_handle, driver))
    }

    pub async fn process_core_request(&mut self,request:CoreRequest) -> Option<E> {
        match request {
            CoreRequest::AddDevice { device} => self.add_device(device).await,
            CoreRequest::RemoveDevice { device } => self.remove_device(device).await,
            CoreRequest::AddPipeconf { pipeconf } => self.add_pipeconf(pipeconf),
            CoreRequest::UpdatePipeconf { device, pipeconf } => self.update_pipeconf(device,pipeconf).await,
            CoreRequest::RemovePipeconf { pipeconf } => self.remove_pipeconf(pipeconf)
        }
    }

    pub fn get_handle(&self) -> Context<E>
        where
            E: Event,
    {
        let conns = self.connections.clone();
        Context::new(
            self.core_channel_sender.clone(),
            self.event_sender.clone(),
            conns,
            self.pipeconf.clone(),
        )
    }

    pub async fn add_device(&mut self, device:Device) -> Option<E> {
        let name = &device.name;
        match device.typ {
            DeviceType::Bmv2MASTER {
                ref socket_addr,
                device_id,
                pipeconf,
            } => {
                if self.connections.contains_key(&device.id) {
                    error!(target:"core","Device with name existed: {:?}",device.name);
                    return None;
                }
                let pipeconf_obj = self.pipeconf.get(&pipeconf);
                if pipeconf_obj.is_none() {
                    error!(target:"core","pipeconf not found: {:?}",pipeconf);
                    return None;
                }
                let pipeconf = pipeconf_obj.unwrap().clone();
                let bmv2connection = Bmv2SwitchConnection::try_new(
                    name,
                    socket_addr,
                    Bmv2ConnectionOption {
                        p4_device_id: device_id,
                        inner_device_id: Some(device.id.0),
                        ..Default::default()
                    },
                )
                    .await;
                if let Err(e) = self.add_bmv2_connection(bmv2connection, &pipeconf).await {
                    error!(target:"core","add {} connection fail: {:?}",name,e);
                    self.event_sender
                        .send(CoreEvent::Event(
                            CommonEvents::DeviceLost(device.id).into_e(),
                        ))
                        .await;
                    return None;
                }
            }
            DeviceType::StratumMASTER {
                ref socket_addr, device_id, pipeconf
            } => {
                if self.connections.contains_key(&device.id) {
                    error!(target:"core","Device with name existed: {:?}",device.name);
                    return None;
                }
                let pipeconf_obj = self.pipeconf.get(&pipeconf);
                if pipeconf_obj.is_none() {
                    error!(target:"core","pipeconf not found: {:?}",pipeconf);
                    return None;
                }
                let pipeconf = pipeconf_obj.unwrap().clone();
                let bmv2connection = StratumBmv2SwitchConnection::try_new(
                    name,
                    socket_addr,
                    StratumBmv2ConnectionOption {
                        p4_device_id: device_id,
                        inner_device_id: Some(device.id.0),
                        ..Default::default()
                    },
                ).await;
                if let Err(e) = self.add_stratum_connection(bmv2connection, &pipeconf).await {
                    error!(target:"core","add {} connection fail: {:?}",name,e);
                    self.event_sender
                        .send(CoreEvent::Event(
                            CommonEvents::DeviceLost(device.id).into_e(),
                        ))
                        .await;
                    return None;
                }
            }
            _ => {}
        }
//        self.event_sender
//            .send(CoreEvent::Event(
//                CommonEvents::DeviceAdded(device.clone()).into_e(),
//            ))
//            .await
//            .unwrap();
        Some(CommonEvents::DeviceAdded(device.clone()).into_e())
    }

    pub async fn remove_device(&mut self, id:DeviceID) -> Option<E> {
        self.connections.remove(&id);
//        self.event_sender
//            .send(CoreEvent::Event(CommonEvents::DeviceLost(id).into_e()))
//            .await
//            .unwrap();
        Some(CommonEvents::DeviceLost(id).into_e())
    }

    pub async fn master_up(&mut self, device:DeviceID, master_update:MasterArbitrationUpdate) -> Result<(), ContextError> {
        let connection = self.connections.get_mut(&device).ok_or(ContextError::from(
            ContextErrorKind::DeviceNotConnected { device },
        ))?;
        connection.master_updated(master_update).await
    }

    pub fn add_pipeconf(&mut self, pipeconf:Pipeconf) -> Option<E> {
        let id = pipeconf.get_id();
        if self.pipeconf.contains_key(&id) {
            return None;
        }
        else {
            Arc::make_mut(&mut self.pipeconf).insert(id, pipeconf);
            return Some(CommonEvents::PipeconfAdded(id).into_e());
        }
    }

    pub fn remove_pipeconf(&mut self, pipeconf:PipeconfID) -> Option<E> {
        Arc::make_mut(&mut self.pipeconf).remove(&pipeconf)?;
        Some(CommonEvents::PipeconfAdded(pipeconf).into_e())
    }

    pub async fn update_pipeconf(&mut self, device:DeviceID, pipeconf:PipeconfID) -> Option<E> {
        let id = pipeconf;
        let pipeconf = if let Some(p) = self.pipeconf.get(&pipeconf) {
          p.clone()
        } else { return None };
        let conn = if let Some(d) = self.connections.get_mut(&device) {
            d
        } else { return None };
        conn.set_pipeconf(pipeconf).await.ok()?;
        Some(CommonEvents::DevicePipeconfUpdate(device,id).into_e())
    }

    pub async fn add_bmv2_connection(
        &mut self,
        mut connection: Bmv2SwitchConnection,
        pipeconf: &Pipeconf,
    ) -> Result<(), ContextError> {
        let (mut request_sender, request_receiver) = tokio::sync::mpsc::channel(4096);
        let mut client = connection.client.clone();
        let mut event_sender = self.event_sender.clone();
        let id = connection.inner_id;
        tokio::spawn(drive_bmv2(client,request_receiver,event_sender,id));

        let master_up_req = crate::p4rt::pure::new_master_update_request(connection.device_id,Bmv2MasterUpdateOption::default());
        request_sender.send(master_up_req).await.unwrap();

        let id = connection.inner_id;
        self.connections.insert(id,
                                Bmv2Connection {
                                    p4runtime_client: connection.client,
                                    sink: request_sender,
                                    device_id: connection.device_id,
                                    pipeconf: pipeconf.clone(),
                                    master_arbitration:None
                                }.clone_box());

        Ok(())
    }
    
    pub async fn add_stratum_connection(
        &mut self,
        mut connection: StratumBmv2SwitchConnection,
        pipeconf: &Pipeconf
    ) -> Result<(), ContextError> {
        let (mut request_sender, request_receiver) = tokio::sync::mpsc::channel(4096);
        let mut client = connection.client.clone();
        let mut event_sender = self.event_sender.clone();
        let id = connection.inner_id;
        tokio::spawn(drive_bmv2(client,request_receiver,event_sender,id));

        let master_up_req = crate::p4rt::pure::new_master_update_request(connection.device_id,Bmv2MasterUpdateOption::default());
        request_sender.send(master_up_req).await.unwrap();

//        let response = crate::p4rt::stratum_bmv2::get_interfaces_name(&mut connection.gnmi_client).await;
//        println!("{:#?}",response);
        // todo: gather device information

        let id = connection.inner_id;
        self.connections.insert(id,
                                StratumBmv2Connection {
                                    p4runtime_client: connection.client,
                                    gnmi_client: connection.gnmi_client,
                                    sink: request_sender,
                                    device_id: connection.device_id,
                                    pipeconf: pipeconf.clone(),
                                    master_arbitration:None
                                }.clone_box());

        Ok(())
    }
}


async fn drive_bmv2<E>(
    mut client:P4RuntimeClient,
    request_receiver:tokio::sync::mpsc::Receiver<StreamMessageRequest>,
    mut event_sender:futures::channel::mpsc::Sender<CoreEvent<E>>,
    id:DeviceID
) {
    let mut response = client.stream_channel(tonic::Request::new(request_receiver)).await.unwrap().into_inner();
    while let Some(Ok(r)) = response.next().await {
        if let Some(update) = r.update {
            match update {
                stream_message_response::Update::Arbitration(masterUpdate) => {
                    debug!(target: "core", "StreaMessageResponse?: {:#?}", &masterUpdate);
                    event_sender.send(CoreEvent::Bmv2MasterUpdate(id,masterUpdate)).await.unwrap();
                }
                stream_message_response::Update::Packet(packet) => {
                    let x = PacketReceived {
                        packet:packet.payload,
                        from: id,
                        metadata: packet.metadata
                    };
                    event_sender.send(CoreEvent::PacketReceived(x)).await.unwrap();
                }
                stream_message_response::Update::Digest(p) => {
                    debug!(target: "core", "StreaMessageResponse: {:#?}", p);
                }
                stream_message_response::Update::IdleTimeoutNotification(n) => {
                    debug!(target: "core", "StreaMessageResponse: {:#?}", n);
                }
                stream_message_response::Update::Other(what) => {
                    debug!(target: "core", "StreaMessageResponse: {:#?}", what);
                }
            }
        }
    }
}
