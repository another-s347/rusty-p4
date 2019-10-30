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
use crate::core::Context;
use crate::event::{CommonEvents, Event};
use std::fmt::Debug;

type P4RuntimeClient =
    crate::proto::p4runtime::client::P4RuntimeClient<tonic::transport::channel::Channel>;

#[derive(Clone)]
pub struct Bmv2Connection {
    pub p4runtime_client: P4RuntimeClient,
    pub sink: tokio::sync::mpsc::Sender<StreamMessageRequest>,
    pub device_id: u64,
    pub pipeconf: Pipeconf,
    pub master_arbitration:Option<MasterArbitrationUpdate>
}

impl Bmv2Connection {
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

    pub async fn master_up<E>(
        &mut self,
        master_update:MasterArbitrationUpdate,
        context:&mut Context<E>
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
        context.send_event(CommonEvents::DevicePipeconfUpdate(self.pipeconf.get_id()).into_e());
        Ok(())
    }
}
