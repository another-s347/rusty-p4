use crate::p4rt::pure::{get_packin_egress_port_metaid, get_packout_egress_port_metaid};
use crate::proto::p4config::P4Info;
use failure::ResultExt;
use log::error;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::sync::Arc;
use std::io::{BufReader, Read};

#[derive(Clone,Debug)]
pub struct Pipeconf {
    id: PipeconfID,
    name: String,
    inner: Arc<Inner>,
    pub packetout_egress_id: u32,
    pub packetin_ingress_id: u32,
}

impl Pipeconf {
    pub fn new<T: AsRef<Path> + Debug>(
        name: &str,
        p4info_file_path: T,
        bmv2_file_path: T,
    ) -> Pipeconf {
        let file = std::fs::File::open(&p4info_file_path);
        if file.is_err() {
            error!(target:"pipeconf", "critical: Opening P4 info file fail: {:?}, path: {:?}", file.err().unwrap(), &p4info_file_path);
            exit(1);
        }
        let mut file = std::fs::File::open(p4info_file_path).unwrap();
        let mut buf = vec![];
        file.read_to_end(&mut buf).unwrap();
        let p4info = prost::Message::decode(buf.as_ref()).unwrap();
        let packetout_id = get_packout_egress_port_metaid(&p4info).unwrap();
        let packetin_id = get_packin_egress_port_metaid(&p4info).unwrap();
        let id = crate::util::hash(name);
        Pipeconf {
            id: PipeconfID(id),
            name: name.to_owned(),
            inner: Arc::new(Inner {
                p4info,
                bmv2_json_file_path: PathBuf::from(bmv2_file_path.as_ref()),
            }),
            packetout_egress_id: packetout_id,
            packetin_ingress_id: packetin_id,
        }
    }

    pub fn get_p4info(&self) -> &P4Info {
        &self.inner.p4info
    }

    pub fn get_bmv2_file_path(&self) -> &Path {
        self.inner.bmv2_json_file_path.as_path()
    }

    pub fn get_id(&self) -> PipeconfID {
        self.id
    }
}

#[derive(Debug)]
struct Inner {
    pub p4info: P4Info,
    pub bmv2_json_file_path: PathBuf,
}

#[derive(Serialize, Deserialize, Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct PipeconfID(pub u64);
