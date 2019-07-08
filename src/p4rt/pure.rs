use log::{debug, error, info, trace, warn};

use crate::error::*;
use crate::proto::p4runtime::TableEntry;
use crate::proto::p4runtime_grpc::P4RuntimeClient;

pub fn write_table_entry(client:&P4RuntimeClient, device_id:u64, table_entry: TableEntry) -> Result<()> {
    let mut request = crate::proto::p4runtime::WriteRequest::new();
    request.set_device_id(device_id);
    request.mut_election_id().low=1;
    let mut update = crate::proto::p4runtime::Update::new();
    if table_entry.is_default_action {
        update.set_field_type(crate::proto::p4runtime::Update_Type::MODIFY);
    }
    else {
        update.set_field_type(crate::proto::p4runtime::Update_Type::INSERT);
    }
    update.mut_entity().mut_table_entry().clone_from(&table_entry);
    request.updates.push(update);
    debug!(target:"write_table_entry", "request: {:#?}", &request);
    client.write(&request)?;

    Ok(())
}