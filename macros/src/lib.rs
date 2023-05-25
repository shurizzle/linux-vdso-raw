use std::collections::HashSet;

use proc_macro::TokenStream;
use proc_macro2::{Delimiter, Group, Ident, Span};
use quote::{quote, ToTokens, TokenStreamExt};

extern crate proc_macro;
extern crate proc_macro2;
extern crate quote;
extern crate syn;

mod generator;
mod parser;

#[derive(Debug, Clone, Copy)]
pub(crate) enum Ver {
    Mandatory(usize),
    Optional(usize),
    None,
}

#[proc_macro]
pub fn vdso(item: TokenStream) -> TokenStream {
    match inner(item) {
        Ok(toks) => toks,
        Err(err) => err.into_compile_error().into(),
    }
}

fn inner(item: TokenStream) -> Result<TokenStream, syn::Error> {
    let (((m_vers, o_vers), (m_fns, o_fns)), mut ts, name) = {
        let s: parser::VdsoStruct = syn::parse(item)?;
        let i = extract_infos(&s)?;
        let name = s.name.clone();
        let ts = s.into_token_stream();
        (i, ts, name)
    };

    ts.append(Ident::new("impl", Span::call_site()));
    ts.append(name);
    ts.append(Group::new(Delimiter::Brace, {
        let mut tokens = quote!(pub(crate) fn from_reader(reader: crate::VdsoReader) -> ::core::option::Option<Self>);
        tokens.append(Group::new(Delimiter::Brace, {
            let mut tokens = quote!(unsafe);
            tokens.append(Group::new(Delimiter::Brace, generator::generate(&m_vers, &o_vers, &m_fns, &o_fns)));
            tokens
        }));
        tokens
    }));

    Ok(ts.into())
}

#[allow(clippy::type_complexity)]
fn extract_infos(
    s: &parser::VdsoStruct,
) -> Result<
    (
        (Vec<Box<str>>, Vec<Box<str>>),
        (
            Vec<(Box<str>, Box<str>, Ver)>,
            Vec<(Box<str>, Box<str>, Ver)>,
        ),
    ),
    syn::Error,
> {
    let mut m_vers = vec![];
    let mut o_vers = vec![];
    for f in &s.fields {
        if let Some(v) = &f.version {
            let v = v.version.value().into_boxed_str();
            if f.optional.is_some() {
                if !m_vers.contains(&v) && !o_vers.contains(&v) {
                    o_vers.push(v);
                }
            } else {
                if let Some(pos) = o_vers.iter().position(|ov| ov.as_ref() == v.as_ref()) {
                    o_vers.remove(pos);
                }

                if !m_vers.contains(&v) {
                    m_vers.push(v);
                }
            }
        }
    }
    m_vers.sort();
    m_vers.dedup();
    o_vers.sort();
    o_vers.dedup();

    let mut m_fns: Vec<(Box<str>, Box<str>, Ver)> = vec![];
    let mut o_fns: Vec<(Box<str>, Box<str>, Ver)> = vec![];

    let mut ident_set = HashSet::new();
    let mut name_set = HashSet::new();
    for f in &s.fields {
        let ident = f.ident.to_string().into_boxed_str();
        if ident_set.contains(&ident) {
            return Err(syn::Error::new(f.ident.span(), "Repeated field"));
        } else {
            ident_set.insert(ident.clone());
        }

        let name = f.name.to_string().into_boxed_str();
        if name_set.contains(&name) {
            return Err(syn::Error::new(f.name.span(), "Repeated vDSO function"));
        } else {
            name_set.insert(name.clone());
        }

        let ver = if let Some(v) = &f.version {
            let v = v.version.value();
            if f.optional.is_some() {
                if let Some(i) = m_vers.iter().position(|v2| v2.as_ref() == v.as_str()) {
                    Ver::Mandatory(i)
                } else if let Some(i) = o_vers.iter().position(|v2| v2.as_ref() == v.as_str()) {
                    Ver::Optional(i)
                } else {
                    unreachable!()
                }
            } else if let Some(i) = m_vers.iter().position(|v2| v2.as_ref() == v.as_str()) {
                Ver::Mandatory(i)
            } else {
                unreachable!()
            }
        } else {
            Ver::None
        };

        if f.optional.is_some() {
            &mut o_fns
        } else {
            &mut m_fns
        }
        .push((ident, name, ver));
    }

    Ok(((m_vers, o_vers), (m_fns, o_fns)))
}
