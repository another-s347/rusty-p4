#[macro_use]
extern crate rusty_p4;

use std::net::Ipv4Addr;
use std::path::Path;
use std::str::FromStr;
use rusty_p4::util::flow::*;
use rusty_p4::util::value::*;

use rusty_p4::p4rt::bmv2::Bmv2SwitchConnection;
use rusty_p4::p4rt::pipeconf::Pipeconf;
use rusty_p4::p4rt::pure::build_table_entry;

pub fn main() {
    let pipeconf = Pipeconf::new(
        "advanced_tunnel",
        "/home/skye/tutorials/exercises/p4runtime/build/advanced_tunnel.p4.p4info.bin",
        "/home/skye/tutorials/exercises/p4runtime/build/advanced_tunnel.json",
    );

    let mut s1 = Bmv2SwitchConnection::new_without_id("s1", "127.0.0.1:50051", 0);
    let mut s2 = Bmv2SwitchConnection::new_without_id("s2", "127.0.0.1:50052", 1);

    s1.master_arbitration_update();
    s2.master_arbitration_update();

    s1.set_forwarding_pipeline_config(pipeconf.get_p4info(), pipeconf.get_bmv2_file_path());
    s2.set_forwarding_pipeline_config(pipeconf.get_p4info(), pipeconf.get_bmv2_file_path());

    write_tunnel_rules(
        &pipeconf,
        &s1,
        &s2,
        100,
        MAC::of("00:00:00:00:02:02"),
        Ipv4Addr::from_str("10.0.2.2").unwrap(),
    );
    write_tunnel_rules(
        &pipeconf,
        &s2,
        &s1,
        200,
        MAC::of("00:00:00:00:01:01"),
        Ipv4Addr::from_str("10.0.1.1").unwrap(),
    );
}

fn write_tunnel_rules(
    pipeconf: &Pipeconf,
    ingress_sw: &Bmv2SwitchConnection,
    egress_sw: &Bmv2SwitchConnection,
    tunnel_id: u32,
    dst_eth_addr: MAC,
    dst_ip_addr: Ipv4Addr,
) {
//    let table_entry = build_table_entry(
//        pipeconf.get_p4info(),
//        "MyIngress.ipv4_lpm",
//        &[("hdr.ipv4.dstAddr", LPM(dst_ip_addr, 32))],
//        false,
//        "MyIngress.myTunnel_ingress",
//        &[("dst_id", encode(tunnel_id))],
//        0,
//        0,
//    );
    let table_entry = flow! {
        pipe="MyIngress";
        table="ipv4_lpm";
        key= {
            "hdr.ipv4.dstAddr"=>dst_ip_addr/32
        };
        action=myTunnel_ingress(dst_id:tunnel_id);
    }.to_table_entry(&pipeconf,0);

    ingress_sw.write_table_entry(dbg!(table_entry));

//    let table_entry = build_table_entry(
//        pipeconf.get_p4info(),
//        "MyIngress.myTunnel_exact",
//        &[("hdr.myTunnel.dst_id", EXACT(tunnel_id))],
//        false,
//        "MyIngress.myTunnel_forward",
//        &[("port", encode(2u32))],
//        0,
//        0,
//    );
    let table_entry = flow! {
        pipe="MyIngress";
        table="myTunnel_exact";
        key= {
            "hdr.myTunnel.dst_id"=>tunnel_id
        };
        action=myTunnel_forward(port:2u32);
    }.to_table_entry(&pipeconf,0);
    ingress_sw.write_table_entry(table_entry);

//    let table_entry = build_table_entry(
//        pipeconf.get_p4info(),
//        "MyIngress.myTunnel_exact",
//        &[("hdr.myTunnel.dst_id", EXACT(tunnel_id))],
//        false,
//        "MyIngress.myTunnel_egress",
//        &[("dstAddr", encode(dst_eth_addr)), ("port", encode(1u32))],
//        0,
//        0,
//    );
    let table_entry = flow! {
        pipe="MyIngress";
        table="myTunnel_exact";
        key= {
            "hdr.myTunnel.dst_id"=>tunnel_id
        };
        action=myTunnel_egress(dstAddr:dst_eth_addr,port:1u32);
    }.to_table_entry(&pipeconf,0);
    egress_sw.write_table_entry(table_entry);
}
