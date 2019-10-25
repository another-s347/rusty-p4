use ipip::ipv4;
use rusty_p4::{flow, flow_match};

#[test]
fn test_empty() {
    flow_match! {};
}

#[test]
fn test_one() {
    let x = Some(3u32);
    let y = 32;
    flow_match! {
        "abcd" => 1u32..2u32
    };
    flow_match! {
        "abcd" => 1u32
    };
    flow_match! {
        "abcd" => 1u32/8
    };
    flow_match! {
        "abcd" => 1u32&2u32
    };
    flow_match! {
        "abcd" => x.unwrap()..2u32
    };
    flow_match! {
        "abcd" => x.unwrap()
    };
    flow_match! {
        "abcd" => x.unwrap()/8
    };
    flow_match! {
        "abcd" => 1u32&y
    };
    flow_match! {
        "abcd" => x.unwrap()/y
    };
    flow_match! {
        "abcd" => x.unwrap()&y
    };
    flow_match! {
        "abcd" => x.unwrap()..1u32
    };
    flow_match! {
        "abcd" => 1u32
    };
    flow_match! {
        "abcd" => 1u32/8
    };
    flow_match! {
        "abcd" => 1u32&2u32
    };
    flow_match! {
        "abcd" => ipv4!(10.0.0.1)/8
    };
}

#[test]
fn test_two() {
    flow_match! {
        "abcd" => 1u32..2u32,
        "abcd" => 1u32,
        "abcd" => 1u32/8,
        "abcd" => 1u32&2u32
    };
}

#[test]
fn test_flow() {
    flow! {
        pipe:"abcd",
        table: "efg" {
            "abcd1" => 1u32..2u32,
            "abcd2" => 1u32,
            "abcd3" => 1u32/8,
            "abcd4" => 1u32&2u32
        }
        action: "Hi" {
            "abcd5": 5,
            "abcd6": 1
        }
        priority:1
    };
}
