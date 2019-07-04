use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use grpcio::{Channel, StreamingCallSink, ClientDuplexReceiver, WriteFlags};
use crate::proto::p4runtime_grpc::P4RuntimeClient;
use crate::proto::p4runtime::{StreamMessageRequest, StreamMessageResponse, TableEntry};
use futures::sink::Sink;
use futures::stream::Stream;
use crate::helper::P4InfoHelper;
use crate::proto::p4info::P4Info;
use std::fs::File;
use std::io::Read;
use crate::proto::p4config::P4DeviceConfig;
use protobuf::Message;

pub struct Bmv2SwitchConnection {
    name:String,
    address:String,
    device_id:u64,
    client:P4RuntimeClient,
    stream_channel_sink:StreamingCallSink<StreamMessageRequest>,
    stream_channel_receiver:ClientDuplexReceiver<StreamMessageResponse>
}

impl Bmv2SwitchConnection {
    pub fn new(name:&str, address:&str, device_id:u64) -> Bmv2SwitchConnection {
        let environment = grpcio::EnvBuilder::new().build();
        let channelBuilder = grpcio::ChannelBuilder::new(Arc::new(environment));
        let channel = channelBuilder.connect(address);

        let client_stub = crate::proto::p4runtime_grpc::P4RuntimeClient::new(channel);

        let (stream_channel_sink,
            stream_channel_receiver
        ) = client_stub.stream_channel().unwrap();

        Bmv2SwitchConnection {
            name: name.to_owned(),
            address: address.to_owned(),
            device_id,
            client: client_stub,
            stream_channel_sink,
            stream_channel_receiver
        }
    }

    pub fn master_arbitration_update(&mut self) {
        let mut request = StreamMessageRequest::new();
        request.mut_arbitration().device_id = self.device_id;
        request.mut_arbitration().mut_election_id().high = 0;
        request.mut_arbitration().mut_election_id().low = 1;
        // TODO: handle async response
        self.stream_channel_sink.start_send((request,WriteFlags::default()));
    }

    pub fn build_device_config(bmv2_json_file_path:&Path) -> P4DeviceConfig {
        let mut device_config = crate::proto::p4config::P4DeviceConfig::new();
        device_config.set_reassign(true);
        let mut file = File::open(bmv2_json_file_path).unwrap();
        let len = file.metadata().unwrap().len() as usize;
        let mut buffer = vec![0u8;len];
        file.read_to_end(&mut buffer);
        device_config.set_device_data(buffer);
        device_config
    }

    pub fn set_forwarding_pipeline_config(&mut self, p4info:&P4Info, bmv2_json_file_path:&Path) {
        let mut request = crate::proto::p4runtime::SetForwardingPipelineConfigRequest::new();
        request.mut_election_id().low = 1;
        request.set_device_id(self.device_id);
        let config = request.mut_config();

        config.mut_p4info().clone_from(p4info);
        config.set_p4_device_config(Self::build_device_config(bmv2_json_file_path).write_to_bytes().unwrap());

        request.set_action(crate::proto::p4runtime::SetForwardingPipelineConfigRequest_Action::VERIFY_AND_COMMIT);
        self.client.set_forwarding_pipeline_config(&request);
    }

    pub fn write_table_entry(&self, table_entry:TableEntry) {
        let mut request = crate::proto::p4runtime::WriteRequest::new();
        request.set_device_id(self.device_id);
        request.mut_election_id().low=1;
        let mut update = crate::proto::p4runtime::Update::new();
        if table_entry.is_default_action {
            update.set_field_type(crate::proto::p4runtime::Update_Type::MODIFY);
        }
        else {
            update.set_field_type(crate::proto::p4runtime::Update_Type::INSERT);
        }
        update.mut_entity().mut_table_entry().clone_from(&table_entry);
        request.updates.push(update);
        self.client.write(&request).unwrap();
    }
}