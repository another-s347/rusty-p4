#![recursion_limit = "128"]

extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2;
use syn::ext::IdentExt;
use syn::parse::{Parse, ParseBuffer, ParseStream, Peek, Result};
use syn::token::Brace;
use syn::{
    braced, parse_macro_input, BinOp, Error, Expr, ExprBlock, ExprCall, ExprGroup, Field, Ident,
    Lit, LitFloat, LitInt, LitStr, Token,
};

use proc_macro_hack::proc_macro_hack;
use quote::quote;
use quote::{ToTokens, TokenStreamExt};
use std::convert::TryInto;
use std::fmt::{Debug, Formatter};
use syn::group::Group;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::token::Token;

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
                    syn::RangeLimits::Closed(_) => unimplemented!("unsupported range limits"),
                    syn::RangeLimits::HalfOpen(_) => {}
                }
                let from: Box<Expr> = range.from.unwrap();
                let to: Box<Expr> = range.to.unwrap();
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
                        return Ok(_FlowMatchValue::Ternary(left, right));
                    }
                    _ => unimplemented!("unsupported op"),
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
        Ok(Self {
            items: input.parse_terminated(_FlowMatchItem::parse)?,
        })
    }
}

fn flow_match_to_quotes(flow_match:_FlowMatch) -> proc_macro2::TokenStream {
    if flow_match.items.is_empty() {
        return quote! {
            <Vec<rusty_p4::util::flow::FlowMatch>>::new()
        };
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

    quote! {
        vec![#(#quotes),*]
    }
}
/*
let flow_match = flow_match!{
    key => value,
    key => value
};
*/
#[proc_macro_hack]
pub fn flow_match(input: TokenStream) -> TokenStream {
    let flow_match = parse_macro_input!(input as _FlowMatch);

    TokenStream::from(flow_match_to_quotes(flow_match))
}

struct _Flow {
    pipe: Option<String>,
    table: String,
    table_match: Punctuated<_FlowMatchItem, Token![,]>,
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
                    if input.parse::<Token![,]>().is_err() {
                        if !input.is_empty() {
                            return Err(syn::Error::new(pipe_name_lit.span(), "Missing ending ,"));
                        }
                        break;
                    }
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
                    let matches = content.parse_terminated(_FlowMatchItem::parse)?;
                    table_matches = Some(matches);
                    input.parse::<Token![,]>();
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
                    let span = p.span().clone();
                    priority = Some(p);
                    if input.parse::<Token![,]>().is_err() {
                        if !input.is_empty() {
                            return Err(syn::Error::new(span, "Missing ending ,"));
                        }
                        break;
                    }
                }
                other => {
                    unimplemented!("Unsupported flow field {}", other);
                }
            }
        }
        if table.is_none() {
            return Err(input.error("Missing table field"));
        }
        if action.is_none() {
            return Err(input.error("Missing action field"));
        }
        Ok(Self {
            pipe,
            table: table.unwrap(),
            table_match: table_matches.unwrap(),
            action_name: action.unwrap(),
            action_parameters: action_params,
            priority,
        })
    }
}

fn action_params_to_quote(params:Option<Punctuated<_FlowActionItem, Token![,]>>) -> proc_macro2::TokenStream {
    if params.is_none() {
        return quote! {
            <Vec<rusty_p4::util::flow::FlowActionParam>>::new()
        };
    }
    let params = params.unwrap();
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

    quote! {
        vec![#(#quotes),*]
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
#[proc_macro_hack]
pub fn flow(input: TokenStream) -> TokenStream {
    let flow = parse_macro_input!(input as _Flow);
    let flow_table_name = flow.pipe.as_ref().map(|pipe|format!("{}.{}",pipe,&flow.table)).unwrap_or(flow.table.clone());
    let action_name = if flow.action_name=="NoAction" {
        flow.action_name
    } else {
        flow.pipe.as_ref().map(|pipe|format!("{}.{}",pipe,flow.action_name)).unwrap_or(flow.action_name)
    };
    let flow_matches = flow_match_to_quotes(_FlowMatch {
        items: flow.table_match
    });
    let action_params = action_params_to_quote(flow.action_parameters);
    let priority = flow.priority.map(|expr|{
        quote!(#expr)
    }).unwrap_or(quote!(1));
    TokenStream::from(quote! {
        rusty_p4::util::flow::Flow {
            table: std::sync::Arc::new(rusty_p4::util::flow::FlowTable {
                name:#flow_table_name,
                matches:#flow_matches
            }),
            action: std::sync::Arc::new(rusty_p4::util::flow::FlowAction {
                name:#action_name,
                params:#action_params
            }),
            priority:#priority,
            metadata:0
        }
    })
}
