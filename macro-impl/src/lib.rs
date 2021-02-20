#![recursion_limit = "128"]

extern crate proc_macro;

use proc_macro::TokenStream;
use syn::{
    braced,
    parse::{Parse, ParseBuffer, Result},
    parse_macro_input, BinOp, Expr, Ident, LitStr, Token,
};

use quote::quote;
use std::fmt::Debug;
use syn::punctuated::Punctuated;

#[derive(Debug)]
struct _FlowMatchItem {
    pub key: String,
    pub value: _FlowMatchValue,
}

#[derive(Debug)]
struct _FlowActionItem {
    pub key: String,
    pub value: Expr,
}

#[derive(Debug)]
enum _FlowMatchValue {
    Exact(Expr),
    Range(Box<Expr>, Box<Expr>),
    Lpm(Box<Expr>, Box<Expr>),
    Ternary(Box<Expr>, Box<Expr>),
}

impl Parse for _FlowMatchValue {
    fn parse(input: &ParseBuffer) -> Result<Self> {
        let expr: Expr = input.parse()?;
        match expr {
            // range
            Expr::Range(range) => {
                match range.limits {
                    syn::RangeLimits::Closed(_) => {
                        return Err(input.error("Unsupported range limits"))
                    }
                    syn::RangeLimits::HalfOpen(_) => {}
                }
                let from: Box<Expr> = range.from.ok_or(input.error("Missing range 'from'"))?;
                let to: Box<Expr> = range.to.ok_or(input.error("Missing range 'to'"))?;
                return Ok(_FlowMatchValue::Range(from, to));
            }
            Expr::Binary(binary) => {
                match binary.op {
                    // ternary
                    BinOp::BitAnd(_) => {
                        let left: Box<Expr> = binary.left;
                        let right: Box<Expr> = binary.right;
                        // compile time check ternary
                        return Ok(_FlowMatchValue::Ternary(left, right));
                    }
                    // lpm
                    BinOp::Div(_) => {
                        let left: Box<Expr> = binary.left;
                        let right: Box<Expr> = binary.right;
                        // compile time check lpm
                        return Ok(_FlowMatchValue::Lpm(left, right));
                    }
                    _ => return Err(input.error("Unsupported operator")),
                }
            }
            // exact
            other => {
                return Ok(_FlowMatchValue::Exact(other));
            } // todo: error on some expr
        }
    }
}

#[derive(Debug)]
struct _FlowMatch {
    pub items: Punctuated<_FlowMatchItem, Token![,]>,
    pub default: Option<Expr>,
}

impl Parse for _FlowMatchItem {
    fn parse(input: &ParseBuffer) -> Result<Self> {
        let key: LitStr = input.parse()?;
        input.parse::<Token![=]>()?;
        input.parse::<Token![>]>()?;
        let value = input.parse()?;
        Ok(Self {
            key: key.value(),
            value,
        })
    }
}

impl Parse for _FlowActionItem {
    fn parse(input: &ParseBuffer) -> Result<Self> {
        let key: LitStr = input.parse()?;
        input.parse::<Token![:]>()?;
        let value = input.parse()?;
        Ok(Self {
            key: key.value(),
            value,
        })
    }
}

impl Parse for _FlowMatch {
    fn parse(input: &ParseBuffer) -> Result<Self> {
        let mut default = None;
        let mut punct = Punctuated::new();
        loop {
            if input.is_empty() {
                break;
            }
            if input.peek(Token![..]) {
                let _ = input.parse::<Token![..]>()?;
                default = Some(input.parse()?);
            } else {
                let value = _FlowMatchItem::parse(input)?;
                if punct
                    .iter()
                    .find(|x: &&_FlowMatchItem| x.key.as_str() == value.key.as_str())
                    .is_some()
                {
                    return Err(input.error(format!("Duplicated match field: {}", value.key)));
                }
                punct.push_value(value);
            }
            if input.is_empty() {
                break;
            }
            let p = input.parse()?;
            punct.push_punct(p);
        }
        let items = punct;
        Ok(Self { items, default })
    }
}

fn flow_match_to_quotes(flow_match: _FlowMatch) -> proc_macro2::TokenStream {
    if flow_match.items.is_empty() && flow_match.default.is_none() {
        if let Some(default) = flow_match.default {
            return quote! {
                #default.clone()
            };
        } else {
            return quote! {
                std::sync::Arc::new(<rusty_p4::util::SmallVec<[rusty_p4::util::flow::FlowMatch;3]>>::new())
            };
        }
    }

    let mut quotes = Vec::with_capacity(flow_match.items.len());
    for m in flow_match.items {
        let name = m.key;
        match m.value {
            _FlowMatchValue::Exact(expr) => {
                quotes.push(quote! {
                    rusty_p4::util::flow::FlowMatch {
                        name: #name,
                        value: rusty_p4::util::value::EXACT(#expr)
                    }
                });
            }
            _FlowMatchValue::Range(from, two) => {
                quotes.push(quote! {
                    rusty_p4::util::flow::FlowMatch {
                        name: #name,
                        value: rusty_p4::util::value::RANGE(#from,#two)
                    }
                });
            }
            _FlowMatchValue::Lpm(v, lpm) => {
                quotes.push(quote! {
                    rusty_p4::util::flow::FlowMatch {
                        name: #name,
                        value: rusty_p4::util::value::LPM(#v,#lpm)
                    }
                });
            }
            _FlowMatchValue::Ternary(v, t) => {
                quotes.push(quote! {
                    rusty_p4::util::flow::FlowMatch {
                        name: #name,
                        value: rusty_p4::util::value::TERNARY(#v,#t)
                    }
                });
            }
        }
    }

    if let Some(d) = flow_match.default {
        quote! {{
            let mut _v: rusty_p4::util::SmallVec<[rusty_p4::util::flow::FlowMatch;3]> = rusty_p4::util::smallvec![#(#quotes),*];
            _v.sort_by(|a,b|a.name.cmp(b.name));
            rusty_p4::util::flow::merge_matches(&mut _v, &#d);
            let _t = std::sync::Arc::new(_v);
            _t
        }}
    } else {
        quote! {{
            let mut _v:rusty_p4::util::SmallVec<[rusty_p4::util::flow::FlowMatch;3]> = rusty_p4::util::smallvec![#(#quotes),*];
            _v.sort_by(|a,b|a.name.cmp(b.name));
            let _t = std::sync::Arc::new(_v);
            _t
        }}
    }
}
/*
let flow_match = flow_match!{
    key => value,
    key => value
};
*/
#[proc_macro]
pub fn flow_match(input: TokenStream) -> TokenStream {
    let flow_match = parse_macro_input!(input as _FlowMatch);

    TokenStream::from(flow_match_to_quotes(flow_match))
}

struct _Flow {
    pipe: Option<String>,
    table: String,
    table_match: _FlowMatch,
    action_name: String,
    action_parameters: Option<Punctuated<_FlowActionItem, Token![,]>>,
    priority: Option<Expr>,
}

impl Parse for _Flow {
    fn parse(input: &ParseBuffer) -> Result<Self> {
        let mut pipe = None;
        let mut table = None;
        let mut table_matches = None;
        let mut action = None;
        let mut action_params = None;
        let mut priority = None;
        while !input.is_empty() {
            let field_name = input.parse::<Ident>()?.to_string();
            match field_name.as_ref() {
                "pipe" => {
                    if pipe.is_some() {
                        return Err(input.error("Duplicated pipe field"));
                    }
                    input.parse::<Token![:]>()?;
                    let pipe_name_lit = input.parse::<LitStr>()?;
                    let pipe_name = pipe_name_lit.value();
                    pipe = Some(pipe_name);
                }
                "table" => {
                    if table.is_some() {
                        return Err(input.error("Duplicated table field"));
                    }
                    input.parse::<Token![:]>()?;
                    let table_name = input.parse::<LitStr>()?.value();
                    let content;
                    braced!(content in input);
                    table = Some(table_name);
                    let matches = _FlowMatch::parse(&content)?;
                    table_matches = Some(matches);
                }
                "action" => {
                    if action.is_some() {
                        return Err(input.error("Duplicated action field"));
                    }
                    input.parse::<Token![:]>()?;
                    let action_name = input.parse::<LitStr>()?.value();
                    action = Some(action_name);
                    if !input.peek(Token![,]) {
                        let content;
                        braced!(content in input);
                        let matches = content.parse_terminated(_FlowActionItem::parse)?;
                        if !matches.is_empty() {
                            action_params = Some(matches);
                        }
                    } else {
                        input.parse::<Token![,]>()?;
                    }
                }
                "priority" => {
                    if priority.is_some() {
                        return Err(input.error("Duplicated priority field"));
                    }
                    input.parse::<Token![:]>()?;
                    let p = input.parse::<Expr>()?;
                    priority = Some(p);
                }
                other => {
                    unimplemented!("Unsupported flow field {}", other);
                }
            }
            if input.parse::<Token![,]>().is_err() {
                if !input.is_empty() {
                    return Err(syn::Error::new(input.span(), "Missing ending ,"));
                }
                break;
            }
        }
        Ok(Self {
            pipe,
            table: table.ok_or(input.error("Missing table field"))?,
            table_match: table_matches.ok_or(input.error("Missing match field"))?,
            action_name: action.ok_or(input.error("Missing action field"))?,
            action_parameters: action_params,
            priority,
        })
    }
}

fn action_params_to_quote(
    params: Option<Punctuated<_FlowActionItem, Token![,]>>,
) -> proc_macro2::TokenStream {
    if let Some(params) = params {
        let mut quotes = Vec::with_capacity(params.len());
        for m in params {
            let name = m.key;
            let expr = m.value;
            quotes.push(quote! {
                rusty_p4::util::flow::FlowActionParam {
                    name: #name,
                    value: rusty_p4::util::value::encode(#expr)
                }
            });
        }

        quote! {{
            let _t:std::sync::Arc<rusty_p4::util::SmallVec<[rusty_p4::util::flow::FlowActionParam;3]>> = std::sync::Arc::new(rusty_p4::util::smallvec![#(#quotes),*]);
            _t
        }}
    } else {
        quote! {
            std::sync::Arc::new(<rusty_p4::util::SmallVec<[rusty_p4::util::flow::FlowActionParam;3]>>::new())
        }
    }
}

/*
let flow = flow!{
    pipe:"",
    table:table_name {
        key => value,
        ley => value
    },
    action:action_name {

    }
}
*/
#[proc_macro]
pub fn flow(input: TokenStream) -> TokenStream {
    let flow = parse_macro_input!(input as _Flow);
    let flow_table_name = flow
        .pipe
        .as_ref()
        .map(|pipe| format!("{}.{}", pipe, &flow.table))
        .unwrap_or(flow.table.clone());
    let action_name = if flow.action_name == "NoAction" {
        flow.action_name
    } else {
        flow.pipe
            .as_ref()
            .map(|pipe| format!("{}.{}", pipe, flow.action_name))
            .unwrap_or(flow.action_name)
    };
    let flow_matches = flow_match_to_quotes(flow.table_match);
    let action_params = action_params_to_quote(flow.action_parameters);
    let priority = flow.priority.map(|expr| quote!(#expr)).unwrap_or(quote!(1));
    TokenStream::from(quote! {
        rusty_p4::util::flow::Flow {
            table: rusty_p4::util::flow::FlowTable {
                name:#flow_table_name,
                matches:#flow_matches
            },
            action: rusty_p4::util::flow::FlowAction {
                name:#action_name,
                params:#action_params
            },
            priority:#priority,
            metadata:0
        }
    })
}
