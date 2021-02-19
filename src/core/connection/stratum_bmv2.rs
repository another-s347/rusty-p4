use crate::p4rt::pipeconf::Pipeconf;
use crate::error::{ContextError, ContextErrorKind};
use failure::ResultExt;
use crate::representation::DeviceID;
use crate::proto::p4runtime::{
    stream_message_response, Entity, Index, MeterEntry, PacketIn, StreamMessageRequest,
    StreamMessageResponse, Uint128, Update, WriteRequest, WriteResponse,
};
use rusty_p4_proto::proto::v1::{
    MasterArbitrationUpdate
};
use rusty_p4_proto::proto::gnmi::SubscribeRequest;
use crate::core::DefaultContext;
use crate::event::{CommonEvents, Event};
use std::fmt::Debug;
use crate::core::connection::{Connection, ConnectionBox};
use std::any::Any;
use async_trait::async_trait;

type P4RuntimeClient =
crate::proto::p4runtime::p4_runtime_client::P4RuntimeClient<tonic::transport::channel::Channel>;
type GNMIClient = rusty_p4_proto::proto::gnmi::g_nmi_client::GNmiClient<tonic::transport::channel::Channel>;

#[derive(Clone)]
pub struct StratumBmv2Connection {
    pub p4runtime_client: P4RuntimeClient,
    pub gnmi_client: GNMIClient,
    pub sink: tokio::sync::mpsc::Sender<StreamMessageRequest>,
    pub device_id: u64,
    pub pipeconf: Pipeconf,
    pub master_arbitration:Option<MasterArbitrationUpdate>
}

impl StratumBmv2Connection {
    pub async fn master_up<E>(
        &mut self,
        master_update:MasterArbitrationUpdate,
        context:&mut DefaultContext<E>
    ) -> Result<(), ContextError>
        where E:Event+Debug
    {
        self.master_arbitration = Some(master_update);
        let request = crate::p4rt::pure::new_set_forwarding_pipeline_config_request(
            self.pipeconf.get_p4info(),
            self.pipeconf.get_bmv2_file_path(),
            self.master_arbitration.as_ref().unwrap(),
            self.device_id).await.context(ContextErrorKind::ConnectionError)?;
        self.p4runtime_client
            .set_forwarding_pipeline_config(tonic::Request::new(request))
            .await.context(ContextErrorKind::ConnectionError)?;
        Ok(())
    }
}

#[async_trait]
impl Connection for StratumBmv2Connection {
    async fn master_updated(&mut self,master_update:MasterArbitrationUpdate) -> Result<(), ContextError> {
        self.master_arbitration = Some(master_update);
        let request = crate::p4rt::pure::new_set_forwarding_pipeline_config_request(
            self.pipeconf.get_p4info(),
            self.pipeconf.get_bmv2_file_path(),
            self.master_arbitration.as_ref().unwrap(),
            self.device_id).await.context(ContextErrorKind::ConnectionError)?;
        self.p4runtime_client
            .set_forwarding_pipeline_config(tonic::Request::new(request))
            .await.context(ContextErrorKind::ConnectionError)?;
        Ok(())
    }

    async fn set_pipeconf(&mut self, pipeconf: Pipeconf) -> Result<(), ContextError> {
        self.pipeconf = pipeconf;
        let request = crate::p4rt::pure::new_set_forwarding_pipeline_config_request(
            self.pipeconf.get_p4info(),
            self.pipeconf.get_bmv2_file_path(),
            self.master_arbitration.as_ref().unwrap(),
            self.device_id).await.context(ContextErrorKind::ConnectionError)?;
        self.p4runtime_client
            .set_forwarding_pipeline_config(tonic::Request::new(request))
            .await.context(ContextErrorKind::ConnectionError)?;
        Ok(())
    }

    fn clone_box(&self) -> ConnectionBox {
        let p4runtime_client = self.p4runtime_client.clone();
        let sink = self.sink.clone();
        let device_id = self.device_id.clone();
        let pipeconf = self.pipeconf.clone();
        let master_arbitration = self.master_arbitration.clone();
        ConnectionBox {
            inner: Box::new(self.clone()),
            p4runtime_client,
            sink,
            device_id,
            pipeconf,
            master_arbitration
        }
    }

    fn as_any(&self) -> &Any {
        self
    }

    fn as_mut_any(&mut self) -> &mut Any {
        self
    }
}