use crate::proto::p4info::P4Info;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone)]
pub struct Pipeconf {
    inner: Arc<Inner>,
    pub packetout_egress_id: u32,
    pub packetin_ingress_id: u32,
}

impl Pipeconf {
    pub fn get_p4info(&self) -> &P4Info {
        &self.inner.p4info
    }

    pub fn get_bmv2_file_path(&self) -> &Path {
        self.inner.bmv2_json_file_path.as_path()
    }
}

struct Inner {
    pub p4info: P4Info,
    pub bmv2_json_file_path: PathBuf,
}

#[derive(Serialize, Deserialize, Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct PipeconfID(pub u64);
