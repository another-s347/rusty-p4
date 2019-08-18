use grpcio_compiler;
use std::env;

fn main() {
    grpcio_compiler::prost_codegen::compile_protos(
        &[
            "../p4runtime/proto/p4/v1/p4runtime.proto",
            "../p4runtime/proto/p4/v1/p4data.proto",
            "../p4runtime/proto/p4/config/v1/p4info.proto",
            "../p4runtime/proto/p4/config/v1/p4types.proto",
            "./p4config.proto",
            "../googleapis/google/rpc/status.proto",
            "../googleapis/google/rpc/code.proto"],
        &["../p4runtime/proto/","../googleapis/","./"],
        "./src/"
    ).unwrap();
}