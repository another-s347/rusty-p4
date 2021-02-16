//mod rust_grpc_out;
//mod rust_out;
//
//pub use rust_grpc_out::p4runtime_grpc as p4runtime_grpc;
//pub use rust_out::status as status;
//pub use rust_out::p4types as p4types;
//pub use rust_out::p4runtime as p4runtime;
//pub use rust_out::p4info as p4info;
//pub use rust_out::p4data as p4data;
//pub use rust_out::p4config as p4config;
//pub use rust_out::code as code;
pub use rusty_p4_proto::proto::config::v1 as p4config;
pub use rusty_p4_proto::proto::v1 as p4runtime;
