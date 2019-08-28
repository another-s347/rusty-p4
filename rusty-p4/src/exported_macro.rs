#[macro_export]
macro_rules! action_param {
    ($($x:tt)*) => {{
        let mut f = Vec::new();
        action_params!(f,$($x)*);
        f
    }};
}

#[macro_export]
macro_rules! action_params {
    ($m:ident,$p:ident:$v:expr,$($t:tt)*) => {
        $m.push($crate::util::flow::FlowActionParam{
            name:stringify!($p),
            value:$crate::util::value::encode($v)
        });
        action_params!($m,$($t)*)
    };
    ($m:ident,$p:ident:$v:expr) => {
        $m.push($crate::util::flow::FlowActionParam{
            name:stringify!($p),
            value:$crate::util::value::encode($v)
        });
    };
    ($m:ident,) => {};
}

#[macro_export]
macro_rules! flow_tablematch {
    ($($x:tt)*) => {{
        let mut f = Vec::with_capacity(5);
        flow_tablematches!(f,$($x)*);
        f
    }};
}

#[macro_export]
macro_rules! flow_tablematches {
    // exact
    ($m:ident,$x:expr=>$y:expr,$($z:tt)*) => {
        $m.push($crate::util::flow::FlowMatch{
            name:$x,
            value:$crate::util::value::EXACT($y)
        });
        flow_tablematches!($m,$($z)*)
    };
    ($m:ident,$x:expr=>$y:expr) => {
        $m.push($crate::util::flow::FlowMatch{
            name:$x,
            value:$crate::util::value::EXACT($y)
        });
    };
    ($m:ident,$x:expr=>$y:ident,$($z:tt)*) => {
        $m.push($crate::util::flow::FlowMatch{
            name:$x,
            value:$crate::util::value::EXACT($y)
        });
        flow_tablematches!($m,$($z)*)
    };
    ($m:ident,$x:expr=>$y:ident) => {
        $m.push($crate::util::flow::FlowMatch{
            name:$x,
            value:$crate::util::value::EXACT($y)
        });
    };
    // exact ip
    ($m:ident,$x:expr=>ip$ip:literal,$($z:tt)*) => {
        $m.push($crate::util::flow::FlowMatch{
            name:$x,
            value:$crate::util::value::EXACT(std::net::IpAddr::from_str($ip).unwrap())
        });
        flow_tablematches!($m,$($z)*)
    };
    ($m:ident,$x:expr=>ip$ip:literal) => {
        $m.push($crate::util::flow::FlowMatch{
            name:$x,
            value:$crate::util::value::EXACT(std::net::IpAddr::from_str($ip).unwrap())
        });
    };
    // exact mac
    ($m:ident,$x:expr=>mac$mac:literal,$($z:tt)*) => {
        $m.push($crate::util::flow::FlowMatch{
            name:$x,
            value:$crate::util::value::EXACT($crate::util::value::MAC::of($ip))
        });
        flow_tablematches!($m,$($z)*)
    };
    ($m:ident,$x:expr=>mac$mac:literal) => {
        $m.push($crate::util::flow::FlowMatch{
            name:$x,
            value:$crate::util::value::EXACT($crate::util::value::MAC::of($ip))
        });
    };
    // lpm
    ($m:ident,$x:expr=>$v:literal/$lpm:literal,$($z:tt)*) => {
        $m.push($crate::util::flow::FlowMatch{
            name:$x,
            value:$crate::util::value::LPM($v,$lpm)
        });
        flow_tablematches!($m,$($z)*)
    };
    ($m:ident,$x:expr=>$v:literal/$lpm:literal) => {
        $m.push($crate::util::flow::FlowMatch{
            name:$x,
            value:$crate::util::value::LPM($v,$lpm)
        });
    };
    ($m:ident,$x:expr=>$v:ident/$lpm:literal,$($z:tt)*) => {
        $m.push($crate::util::flow::FlowMatch{
            name:$x,
            value:$crate::util::value::LPM($v,$lpm)
        });
        flow_tablematches!($m,$($z)*)
    };
    ($m:ident,$x:expr=>$v:ident/$lpm:literal) => {
        $m.push($crate::util::flow::FlowMatch{
            name:$x,
            value:$crate::util::value::LPM($v,$lpm)
        });
    };
    // lpm ip
    ($m:ident,$x:expr=>ip$ip:literal/$lpm:literal,$($z:tt)*) => {
        $m.push($crate::util::flow::FlowMatch{
            name:$x,
            value:$crate::util::value::LPM(std::net::IpAddr::from_str($ip).unwrap(),$lpm)
        });
        flow_tablematches!($m,$($z)*)
    };
    ($m:ident,$x:expr=>ip$ip:literal/$lpm:literal) => {
        $m.push($crate::util::flow::FlowMatch{
            name:$x,
            value:$crate::util::value::LPM(std::net::IpAddr::from_str($ip).unwrap(),$lpm)
        });
    };
    // lpm mac
    ($m:ident,$x:expr=>mac$mac:literal/$lpm:literal,$($z:tt)*) => {
        $m.push($crate::util::flow::FlowMatch{
            name:$x,
            value:$crate::util::value::LPM($crate::util::value::MAC::of($mac),$lpm)
        });
        flow_tablematches!($m,$($z)*)
    };
    ($m:ident,$x:expr=>mac$mac:literal/$lpm:literal) => {
        $m.push($crate::util::flow::FlowMatch{
            name:$x,
            value:$crate::util::value::LPM($crate::util::value::MAC::of($mac),$lpm)
        });
    };
    // ternary
    ($m:ident,$x:expr=>$v:literal&$ternary:literal,$($z:tt)*) => {
        $m.push($crate::util::flow::FlowMatch{
            name:$x,
            value:$crate::util::value::TERNARY($v,$ternary)
        });
        flow_tablematches!($m,$($z)*)
    };
    ($m:ident,$x:expr=>$v:literal&$ternary:literal) => {
        $m.push($crate::util::flow::FlowMatch{
            name:$x,
            value:$crate::util::value::TERNARY($v,$ternary)
        });
    };
    ($m:ident,$x:expr=>$v:ident&$ternary:literal,$($z:tt)*) => {
        $m.push($crate::util::flow::FlowMatch{
            name:$x,
            value:$crate::util::value::TERNARY($v,$ternary)
        });
        flow_tablematches!($m,$($z)*)
    };
    ($m:ident,$x:expr=>$v:ident&$ternary:literal) => {
        $m.push($crate::util::flow::FlowMatch{
            name:$x,
            value:$crate::util::value::TERNARY($v,$ternary)
        });
    };
    // ternary ip
    ($m:ident,$x:expr=>ip$ip:literal&$ternary:literal,$($z:tt)*) => {
        use std::str::FromStr;
        $m.push($crate::util::flow::FlowMatch{
            name:$x,
            value:$crate::util::value::TERNARY(std::net::IpAddr::from_str($ip).unwrap(),$ternary)
        });
        flow_tablematches!($m,$($z)*)
    };
    ($m:ident,$x:expr=>ip$ip:literal&$ternary:literal) => {
        use std::str::FromStr;
        $m.push($crate::util::flow::FlowMatch{
            name:$x,
            value:$crate::util::value::TERNARY(std::net::IpAddr::from_str($ip).unwrap(),$ternary)
        });
    };
    // ternary mac
    ($m:ident,$x:expr=>mac$mac:literal&$ternary:literal,$($z:tt)*) => {
        $m.push($crate::util::flow::FlowMatch{
            name:$x,
            value:$crate::util::value::TERNARY($crate::util::value::MAC::of($mac),$ternary)
        });
        flow_tablematches!($m,$($z)*)
    };
    ($m:ident,$x:expr=>mac$mac:literal&$ternary:literal) => {
        $m.push($crate::util::flow::FlowMatch{
            name:$x,
            value:$crate::util::value::TERNARY($crate::util::value::MAC::of($mac),$ternary)
        });
    };
    // range literal..literal
    ($m:ident,$x:expr=>$v:literal to $p:literal,$($z:tt)*) => {
        $m.push($crate::util::flow::FlowMatch{
            name:$x,
            value:$crate::util::value::RANGE($v,$p)
        });
        flow_tablematches!($m,$($z)*)
    };
    ($m:ident,$x:expr=>$v:literal to $p:literal) => {
        $m.push($crate::util::flow::FlowMatch{
            name:$x,
            value:$crate::util::value::RANGE($v,$p)
        });
    };
    // range ident..literal
    ($m:ident,$x:expr=>$v:ident to $p:literal,$($z:tt)*) => {
        $m.push($crate::util::flow::FlowMatch{
            name:$x,
            value:$crate::util::value::RANGE($v,$p)
        });
        flow_tablematches!($m,$($z)*)
    };
    ($m:ident,$x:expr=>$v:ident to $p:literal) => {
        $m.push($crate::util::flow::FlowMatch{
            name:$x,
            value:$crate::util::value::RANGE($v,$p)
        });
    };
    // range ident..ident
    ($m:ident,$x:expr=>$v:ident to $p:ident,$($z:tt)*) => {
        $m.push($crate::util::flow::FlowMatch{
            name:$x,
            value:$crate::util::value::RANGE($v,$p)
        });
        flow_tablematches!($m,$($z)*)
    };
    ($m:ident,$x:expr=>$v:ident to $p:ident) => {
        $m.push($crate::util::flow::FlowMatch{
            name:$x,
            value:$crate::util::value::RANGE($v,$p)
        });
    };
    // range literal..ident
    ($m:ident,$x:expr=>$v:literal to $p:ident,$($z:tt)*) => {
        $m.push($crate::util::flow::FlowMatch{
            name:$x,
            value:$crate::util::value::RANGE($v,$p)
        });
        flow_tablematches!($m,$($z)*)
    };
    ($m:ident,$x:expr=>$v:literal to $p:ident) => {
        $m.push($crate::util::flow::FlowMatch{
            name:$x,
            value:$crate::util::value::RANGE($v,$p)
        });
    };
}

#[macro_export]
macro_rules! flow {
    (
    pipe=$pipe:expr;
    table=$table:expr;
    key={$($t:tt)*};
    action=$action_name:ident($($pt:tt)*);
    priority=$priority:expr$(;)?
    ) => {
        $crate::util::flow::Flow {
            table: std::sync::Arc::new($crate::util::flow::FlowTable{
                name:concat!($pipe,'.',$table),
                matches:flow_tablematch!($($t)*)
            }),
            action: std::sync::Arc::new($crate::util::flow::FlowAction {
                name:stringify!($action_name),
                params:action_param!($($pt)*)
            }),
            metadata:0,
            priority:$priority
        }
    };
    (
    pipe=$pipe:expr;
    table=$table:expr;
    key={$($t:tt)*};
    action=$action_name:ident($($pt:tt)*)$(;)?
    ) => {
        $crate::util::flow::Flow {
            table: std::sync::Arc::new($crate::util::flow::FlowTable{
                name:concat!($pipe,'.',$table),
                matches:flow_tablematch!($($t)*)
            }),
            action: std::sync::Arc::new($crate::util::flow::FlowAction {
                name:stringify!($action_name),
                params:action_param!($($pt)*)
            }),
            metadata:0,
            priority:0
        }
    };
}

/*
flow!{
    table = xxxx,
    key = {
        aaaa=>0x1234
        bbbb=>0x1234&0x1234
        cccc=>0x1234..0x5678
    },
    action = action_name(abcd:0x1234,xyz:1234)
}
*/

#[test]
fn test_macro() {
    let flow = flow! {
        pipe="MyIngress";
        table = "xxxx";
        key = {
             "aaaa"=>ip"10.0.0.1"&0b1111,
             "bbbb"=>0x123 to 0x456
        };
        action = action_name(abcd:0x1234,xyz:0x445)
    };
    println!("{:#?}", flow);
}
