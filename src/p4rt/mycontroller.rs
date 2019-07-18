use std::net::Ipv4Addr;
use std::path::Path;
use std::str::FromStr;

use crate::util::value::*;

use super::bmv2::Bmv2SwitchConnection;
use super::helper::P4InfoHelper;

pub fn run() {
    let p4info_helper = P4InfoHelper::new(&Path::new(
        "/home/skye/tutorials/exercises/p4runtime/build/advanced_tunnel.p4.p4info.bin",
    ));
    let bmv2_file = "/home/skye/tutorials/exercises/p4runtime/build/advanced_tunnel.json";
    let mut s1 = Bmv2SwitchConnection::new("s1", "127.0.0.1:50051", 0);
    let mut s2 = Bmv2SwitchConnection::new("s2", "127.0.0.1:50052", 1);

    s1.master_arbitration_update_async();
    s2.master_arbitration_update_async();

    s1.set_forwarding_pipeline_config(&p4info_helper.p4info, &Path::new(bmv2_file));
    s2.set_forwarding_pipeline_config(&p4info_helper.p4info, &Path::new(bmv2_file));

    write_tunnel_rules(
        &p4info_helper,
        &s1,
        &s2,
        100,
        MAC::of("00:00:00:00:02:02"),
        Ipv4Addr::from_str("10.0.2.2").unwrap(),
    );
    write_tunnel_rules(
        &p4info_helper,
        &s2,
        &s1,
        200,
        MAC::of("00:00:00:00:01:01"),
        Ipv4Addr::from_str("10.0.1.1").unwrap(),
    );
}

fn write_tunnel_rules(
    p4info_helper: &P4InfoHelper,
    ingress_sw: &Bmv2SwitchConnection,
    egress_sw: &Bmv2SwitchConnection,
    tunnel_id: u32,
    dst_eth_addr: MAC,
    dst_ip_addr: Ipv4Addr,
) {
    let table_entry = p4info_helper.build_table_entry(
        "MyIngress.ipv4_lpm",
        &[("hdr.ipv4.dstAddr", Value::LPM(dst_ip_addr, 32))],
        false,
        "MyIngress.myTunnel_ingress",
        &[("dst_id", ParamValue::of(tunnel_id))],
        0,
    );

    ingress_sw.write_table_entry(dbg!(table_entry));

    let table_entry = p4info_helper.build_table_entry(
        "MyIngress.myTunnel_exact",
        &[("hdr.myTunnel.dst_id", Value::EXACT(tunnel_id))],
        false,
        "MyIngress.myTunnel_forward",
        &[("port", ParamValue::of(2u32))],
        0,
    );
    ingress_sw.write_table_entry(table_entry);

    let table_entry = p4info_helper.build_table_entry(
        "MyIngress.myTunnel_exact",
        &[("hdr.myTunnel.dst_id", Value::EXACT(tunnel_id))],
        false,
        "MyIngress.myTunnel_egress",
        &[
            ("dstAddr", ParamValue::with(dst_eth_addr.encode())),
            ("port", ParamValue::of(1u32)),
        ],
        0,
    );
    egress_sw.write_table_entry(table_entry);
}
