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

pub trait Pipeconf: Send+Sync {
    fn get_id(&self) -> PipeconfID;
    fn get_name(&self) -> &str;
    fn get_p4info(&self) -> &P4Info;
    fn get_bmv2_file_path(&self) -> &Path;
    fn get_behaviour(&self, name: &str) -> Box<dyn Behaviour>;
    fn get_packetin_ingress_id(&self) -> u32;
    fn get_packetout_egress_id(&self) -> u32;
}

impl Pipeconf for &Arc<dyn Pipeconf> {
    fn get_id(&self) -> PipeconfID {
        self.as_ref().get_id()
    }

    fn get_name(&self) -> &str {
        self.as_ref().get_name()
    }

    fn get_p4info(&self) -> &P4Info {
        self.as_ref().get_p4info()
    }

    fn get_bmv2_file_path(&self) -> &Path {
        self.as_ref().get_bmv2_file_path()
    }

    fn get_behaviour(&self, name: &str) -> Box<dyn Behaviour> {
        self.as_ref().get_behaviour(name)
    }

    fn get_packetin_ingress_id(&self) -> u32 {
        self.as_ref().get_packetin_ingress_id()
    }

    fn get_packetout_egress_id(&self) -> u32 {
        self.as_ref().get_packetout_egress_id()
    }
}

pub trait Behaviour {

}

#[derive(Clone,Debug)]
pub struct DefaultPipeconf {
    id: PipeconfID,
    name: String,
    inner: Arc<Inner>,
    pub packetout_egress_id: u32,
    pub packetin_ingress_id: u32,
}

impl DefaultPipeconf {
    pub fn new<T: AsRef<Path> + Debug>(
        name: &str,
        p4info_file_path: T,
        bmv2_file_path: T,
    ) -> DefaultPipeconf {
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
        DefaultPipeconf {
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

impl Pipeconf for DefaultPipeconf {
    fn get_id(&self) -> PipeconfID {
        self.id
    }

    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_p4info(&self) -> &P4Info {
        &self.inner.p4info
    }

    fn get_bmv2_file_path(&self) -> &Path {
        &self.inner.bmv2_json_file_path
    }

    fn get_behaviour(&self, name: &str) -> Box<dyn Behaviour> {
        todo!()
    }

    fn get_packetin_ingress_id(&self) -> u32 {
        self.packetin_ingress_id
    }

    fn get_packetout_egress_id(&self) -> u32 {
        self.packetout_egress_id
    }
}