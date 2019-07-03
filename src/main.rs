use std::path::Path;
use crate::bmv2::Bmv2SwitchConnection;
use crate::helper::P4InfoHelper;

mod proto;
mod mycontroller;
mod helper;
mod bmv2;

fn main() {
    let p4info_helper = helper::P4InfoHelper::new(&Path::new("123"));

    let mut s1 = Bmv2SwitchConnection::new("s1","127.0.0.1:50051",0);

    s1.master_arbitration_update();

    s1.set_forwarding_pipeline_config(&p4info_helper.p4info,&Path::new("bmv2_file_path"));

}

fn write_tunnel_rules(p4info_helper:P4InfoHelper, ingress_sw:Bmv2SwitchConnection, egress_sw:Bmv2SwitchConnection,
                      tunnel_id:u32, dst_eth_addr:&str, dst_ip_addr:&str)
{
//    let table_entry = p4info_helper.
}
