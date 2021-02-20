use futures::stream::StreamExt;
use p4rt::{
    bmv2::{Bmv2ConnectionOption, Bmv2MasterUpdateOption},
    pipeconf::DefaultPipeconf,
};
use rusty_p4::p4rt::{self, bmv2::Bmv2SwitchConnection};
use std::sync::Arc;
use tokio;

#[tokio::main]
pub async fn main() {
    let pipeconf = DefaultPipeconf::new(
        "my_pipeconf",
        "./pipeconf/my_pipeconf.p4.p4info.bin",
        "./pipeconf/my_pipeconf.json",
    );

    // connect to switch
    let mut conn = Bmv2SwitchConnection::new(
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
    )
    .await
    .unwrap();

    // open bi-stream, which send election request
    let _sender = conn.open_stream().await.unwrap();

    // take the stream receiver
    let mut receiver = conn.take_channel_receiver().unwrap();

    // process response
    while let Some(Ok(r)) = receiver.next().await {
        match r.update.unwrap() {
            rusty_p4::proto::p4runtime::stream_message_response::Update::Arbitration(update) => {
                // if we are the master, set pipeline config.
                if conn.set_master(update).is_ok() {
                    conn.set_forwarding_pipeline_config(Arc::new(pipeconf.clone()))
                        .await
                        .unwrap();
                }
            }
            other => {
                println!("{:?}", other);
            }
        }
    }
}
