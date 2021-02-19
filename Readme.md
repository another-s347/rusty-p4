# Work in Progress


I hope I can finish this.

# Rusty P4 [![Build (Linux)](https://github.com/another-s347/rusty-p4/actions/workflows/build_linux.yml/badge.svg?branch=new_app&event=push)](https://github.com/another-s347/rusty-p4/actions/workflows/build_linux.yml) [![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0) [![dev doc](https://img.shields.io/badge/-dev%20doc-ff69b4)](https://another-s347.github.io/docs/rusty_p4/rusty_p4/)
A Work-in-progress composable and lightweight library for writing [P4Runtime](https://p4.org/specs/) controller in Rust. The goal is to bring powerful & expressive tools into the world of P4 and SDN so that developers can test their ideas faster.

It's trying to provide multi-level APIs for writing controllers with different complexity (see Examples below). Some design come from the tutorials of P4 and [ONOS](https://onosproject.org/).

## Repo structure

- rusty-p4-core. The core of rusty-p4, providing the basic building blocks like App trait, Service trait, P4 runtime, Pipeconf, flow and some other things.
- rusty-p4-packet. Provides methods to parse packets.
- rusty-p4-northbound. Provides impls for northbound server.
- rusty-p4-app. Provides some simple application implementation.

## Getting Started

Current version hasn't been published, so to use it, add
```
rusty-p4 = { git="https://github.com/another-s347/rusty-p4" }
```
to your Cargo.toml.

## Examples

TODO
<!-- 1. [tutorials from P4](https://github.com/p4lang/tutorials/blob/master/exercises/p4runtime/mycontroller.py) (low-level API) (See src/p4rt/mycontroller.rs)
```rust
pub fn run() {
    let p4info_helper = P4InfoHelper::new(&Path::new("path_to/advanced_tunnel.p4.p4info.bin"));
    let bmv2_file = "path_to/advanced_tunnel.json";
    let mut s1 = Bmv2SwitchConnection::new("s1","127.0.0.1:50051",0);
    let mut s2 = Bmv2SwitchConnection::new("s2","127.0.0.1:50052",1);

    s1.master_arbitration_update_async();
    s2.master_arbitration_update_async();

    s1.set_forwarding_pipeline_config(&p4info_helper.p4info,&Path::new(bmv2_file));
    s2.set_forwarding_pipeline_config(&p4info_helper.p4info,&Path::new(bmv2_file));

    write_tunnel_rules(&p4info_helper, &s1, &s2, 100, MACString("00:00:00:00:02:02".to_owned()), Ipv4Addr::from_str("10.0.2.2").unwrap());
    write_tunnel_rules(&p4info_helper, &s2, &s1, 200, MACString("00:00:00:00:01:01".to_owned()), Ipv4Addr::from_str("10.0.1.1").unwrap());
}

fn write_tunnel_rules(p4info_helper:&P4InfoHelper, ingress_sw:&Bmv2SwitchConnection, egress_sw:&Bmv2SwitchConnection,
                      tunnel_id:u32, dst_eth_addr:MACString, dst_ip_addr:Ipv4Addr)
{
    let table_entry = p4info_helper.build_table_entry(
        "MyIngress.ipv4_lpm",
        &[
            ("hdr.ipv4.dstAddr", Value::LPM(dst_ip_addr, 32))
        ],
        false,
        "MyIngress.myTunnel_ingress",
        &[
            ("dst_id", ParamValue::of(tunnel_id))
        ],
        0
    );

    ingress_sw.write_table_entry(dbg!(table_entry));
}
```
2. Packet Counter (mid-level API)
```rust
pub struct Example {
    pub counter:u32
}

impl P4app for Example {
    fn on_packet(self:&mut Self, packet:PacketReceived, ctx: &ContextHandle) {
        let packet = Bytes::from(packet.packet.payload);
        let parsed:Option<Ethernet<Data>> = Ethernet::from_bytes(packet);
        if let Some(ethernet) = parsed {
            self.counter+=1;
            println!("Counter == {}, ethernet type == {:x}", self.counter, ethernet.ether_type);
        }
        else {
            println!("packet parse fail");
        }
    }
}
```
3. ... (high-level API)
```rust
pub struct AdhocApp {
    flowMap: HashMap<String, FlowOwned>
}

impl P4appExtended<CommonEvents> for AdhocApp {
    fn on_packet(self: &mut Self, packet: PacketReceived, ctx: &ContextHandle<CommonEvents>, state: &CommonState) {
        if let Some(eth) = Ethernet::from_bytes(BytesMut::from(packet.packet.payload)) {
            if eth.ether_type == 0x865 {
                let path = state.graph.get_path(...);
            }
        }
    }

    fn on_host_added(self: &mut Self, host: &Host, state: &CommonState, ctx: &ContextHandle<CommonEvents>) {

    }

    fn on_device_added(self: &mut Self, device: &Device, state: &CommonState, ctx: &ContextHandle<CommonEvents>) {

    }

    fn on_link_added(self: &mut Self, link: &Link, state: &CommonState, ctx: &ContextHandle<CommonEvents>) {

    }
}
``` -->
<!-- ## TODO
0. Migrate to tokio 0.2/ hyper 0.13/ tower-grpc.
1. Complete P4Runtime API (read/write table, counter...).
2. More packet parser.
3. ~~Composable App~~.
4. ~~Extended-App and app collection for high-level API~~.
5. Logging and error handling and config.
6. ~~Network-object configuration~~(Simple net config supported).
7. ~~State restore~~.
8. ~~Multiple p4 pipeline~~(need test and app update).
9. Rest API for external control (CLI and ONOS restconf driver..etc).
10. More API.
11. Maybe more. -->

## Built With

* [tonic](https://github.com/hyperium/tonic) - gRPC for Rust.
* [PI](https://github.com/p4lang/PI) - P4Runtime
* [tokio](https://tokio.rs) - The asynchronous run-time for the Rust programming language.