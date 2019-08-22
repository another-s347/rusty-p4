pub mod proto {
    pub mod v1 {
        include!("p4.v1.rs");
    }

    pub mod config {
        pub mod v1 {
            include!("p4.config.v1.rs");
        }
    }
}

pub mod google {
    pub mod rpc {
        include!("google.rpc.rs");
    }
}
