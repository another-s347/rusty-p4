use crate::core::{Core, Context};
use crate::proto::p4runtime::{
    stream_message_response, Entity, Index, MeterEntry, PacketIn, StreamMessageRequest,
    StreamMessageResponse, Uint128, Update, WriteRequest, WriteResponse,
};
use std::sync::Arc;
use crate::error::{ContextError, ContextErrorKind};
use crate::representation::DeviceID;
use crate::event::{Event, CommonEvents};
use std::fmt::Debug;
use failure::ResultExt;
use crate::p4rt::pipeconf::Pipeconf;
use rusty_p4_proto::proto::v1::{
    MasterArbitrationUpdate
};
use async_trait::async_trait;
use std::any::Any;

pub mod bmv2;
pub mod stratum_bmv2;

type P4RuntimeClient =
crate::proto::p4runtime::client::P4RuntimeClient<tonic::transport::channel::Channel>;

#[async_trait]
pub trait Connection:Send+Sync+'static {
    async fn master_updated(&mut self,master_update:MasterArbitrationUpdate) -> Result<(), ContextError>;

    async fn set_pipeconf(&mut self, pipeconf:Pipeconf) -> Result<(), ContextError>;

    fn clone_box(&self)->ConnectionBox;

    fn as_any(&self) -> &dyn Any;

    fn as_mut_any(&mut self) -> &mut dyn Any;
}

pub struct ConnectionBox {
    // use arc?
    pub(crate) inner:Box<dyn Connection>,
    pub p4runtime_client: P4RuntimeClient,
    pub sink: tokio::sync::mpsc::Sender<StreamMessageRequest>,
    pub device_id: u64,
    pub pipeconf: Pipeconf,
    pub master_arbitration:Option<MasterArbitrationUpdate>
}

impl ConnectionBox
{
    pub async fn send_stream_request(
        &mut self,
        request: StreamMessageRequest,
    ) -> Result<(), ContextError> {
        self.sink
            .send(request)
            .await
            .context(ContextErrorKind::DeviceNotConnected {
                device: DeviceID(self.device_id),
            })?;

        Ok(())
    }

    pub async fn send_request(
        &mut self,
        request: WriteRequest,
    ) -> Result<WriteResponse, ContextError> {
        Ok(self
            .p4runtime_client
            .write(tonic::Request::new(request))
            .await
            .context(ContextErrorKind::ConnectionError)?
            .into_inner())
    }

    pub async fn master_updated(
        &mut self,
        master_update:MasterArbitrationUpdate
    ) -> Result<(), ContextError>
    {
        self.master_arbitration = Some(master_update.clone());
        self.inner.master_updated(master_update).await
//todo:        context.send_event(CommonEvents::DevicePipeconfUpdate(self.pipeconf.get_id()).into_e());
    }

    pub async fn set_pipeconf(&mut self, pipeconf:Pipeconf) -> Result<(), ContextError> {
        self.pipeconf = pipeconf.clone();
        self.inner.set_pipeconf(pipeconf).await
    }

    pub fn get_inner<T:Connection+Clone>(&self) -> Option<&T> {
        let o = self.inner.as_ref().as_any();
        o.downcast_ref::<T>()
    }

    pub fn get_inner_mut<T:Connection+Clone>(&mut self) -> Option<&mut T> {
        let o = self.inner.as_mut().as_mut_any();
        o.downcast_mut::<T>()
    }
}

impl Clone for ConnectionBox {
    fn clone(&self) -> Self {
        self.inner.clone_box()
    }
}