use std::collections::HashMap;

pub fn new_gnmi_path(path:&str) -> rusty_p4_proto::proto::gnmi::Path {
    let elems:Vec<&str> = path.split('/').collect();
    let mut value = vec![];
    for elem in elems {
        if elem.contains('[') {
            let mut t:Vec<&str> = elem.split('[').collect();
            let mut map = HashMap::new();
            let name = t.remove(0).to_string();
            for kv in t {
                let mut kv:Vec<&str> = kv.split('=').collect();
                let key = kv.remove(0);
                let value = kv.remove(0);
                let (value,_) = value.split_at(value.len()-1);
                map.insert(key.to_string(),value.to_string());
            }
            value.push(rusty_p4_proto::proto::gnmi::PathElem {
                name,
                key:map
            });
        }
        else {
            if !elem.is_empty() {
                value.push(rusty_p4_proto::proto::gnmi::PathElem {
                    name:elem.to_string(),
                    ..Default::default()
                })
            }
        }
    }
    rusty_p4_proto::proto::gnmi::Path {
        elem:value,
        ..Default::default()
    }
}

#[test]
fn test_new_gnmi_path() {
    dbg!(new_gnmi_path("/interfaces/interface[name=*][test=*]"));
    dbg!(new_gnmi_path("/interfaces/interface[name=*]"));
    dbg!(new_gnmi_path("/"));
}