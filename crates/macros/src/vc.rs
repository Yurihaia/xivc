use std::collections::HashMap;

use proc_macro2::{Ident, TokenStream};
use quote::{quote, ToTokens};
use syn::{
    parse::{Parse, ParseStream, Parser},
    Attribute, Expr, Fields, ItemEnum, Meta, Token, Type, Visibility,
};

pub fn var_const_impl(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let mut item: ItemEnum = syn::parse2(item)?;
    let item_name = &item.ident;

    // done to preserve const ordering, which is noticable in documentation.
    let (mut consts, names) = Parser::parse2(
        |input: ParseStream| {
            let mut names = HashMap::new();
            let mut consts = Vec::new();
            while !input.is_empty() {
                let item: VarConstAttrItem = input.parse()?;
                if names.contains_key(&item.id) {
                    let msg = format!("the constant `{}` is defined multiple times", item.id);
                    return Err(syn::Error::new_spanned(item.id, msg));
                }
                names.insert(item.id.clone(), consts.len());
                consts.push((item, TokenStream::new()));
            }
            Ok((consts, names))
        },
        attr,
    )?;

    for x in &mut item.variants {
        let skip = match x.fields {
            Fields::Unit => TokenStream::new(),
            Fields::Unnamed(..) => quote! { ( .. ) },
            Fields::Named(..) => quote! { { .. } },
        };
        let mut kept = Vec::with_capacity(x.attrs.len());
        for a in x.attrs.drain(..) {
            let var_id = &x.ident;
            if let Some(id) = a.path().get_ident().cloned() {
                if let Some(i) = names.get(&id) {
                    if let Meta::List(..) = &a.meta {
                        return Err(syn::Error::new(
                            a.bracket_token.span.join(),
                            format!("cannot use a list attribute for `{}`.", id),
                        ));
                    }
                    let (c, cs) = &mut consts[*i];
                    match &c.item_type {
                        ConstType::Flag { proxy } => match a.meta {
                            Meta::Path(_) => {
                                if let Some((_, real, _, def)) = proxy {
                                    let def = def.clone();
                                    let mut real = real.clone();
                                    real.set_span(id.span());
                                    let Some((src, cs)) = names.get(&real).map(|v| &mut consts[*v])
                                    else {
                                        let msg = format!("unknown constant name `{}`", real);
                                        return Err(syn::Error::new_spanned(real, msg));
                                    };
                                    let ret = match &src.item_type {
                                        ConstType::Value { optional, .. } => {
                                            if optional.is_some() {
                                                quote! { ::core::option::Option::Some( #def ) }
                                            } else {
                                                quote! { #def }
                                            }
                                        }
                                        ConstType::Flag { proxy } => {
                                            if proxy.is_some() {
                                                let msg = format!("cannot create a proxy constant for proxy constant `{}`", real);
                                                return Err(syn::Error::new_spanned(id, msg));
                                            }
                                            quote! { true }
                                        }
                                    };
                                    cs.extend(quote! {
                                        Self::#var_id #skip => {
                                            let _ = Self::#real;
                                            #ret
                                        }
                                    });
                                } else {
                                    cs.extend(quote! {
                                        Self::#var_id #skip => {
                                             // gives r-a hover info. such a dumb hack
                                            let _ = Self::#id;
                                            true
                                        }
                                    });
                                }
                            }
                            _ => {
                                return Err(syn::Error::new(
                                    a.bracket_token.span.join(),
                                    format!("cannot use a name-value attribute for `{}`", id),
                                ))
                            }
                        },
                        ConstType::Value {
                            optional, default, ..
                        } => match a.meta {
                            Meta::Path(_) => {
                                if optional.is_some() {
                                    if let Some((_, def)) = default {
                                        cs.extend(quote! {
                                            Self::#var_id #skip => {
                                                let _ = Self::#id;
                                                ::core::option::Option::Some(#def)
                                            }
                                        });
                                        continue;
                                    }
                                }
                                return Err(syn::Error::new(
                                    a.bracket_token.span.join(),
                                    format!("cannot use a flag attribute for `{}`", id),
                                ));
                            }
                            Meta::NameValue(nv) => {
                                let val = nv.value;
                                if optional.is_some() {
                                    cs.extend(quote! {
                                        Self::#var_id #skip => {
                                            let _ = Self::#id;
                                            ::core::option::Option::Some(#val)
                                        }
                                    });
                                } else {
                                    cs.extend(quote! {
                                        Self::#var_id #skip => {
                                            let _ = Self::#id;
                                            #val
                                        }
                                    });
                                }
                            }
                            _ => unreachable!(),
                        },
                    }
                    continue;
                }
            }
            kept.push(a);
        }
        x.attrs = kept;
    }

    let mut other = TokenStream::new();

    let ci = consts
        .into_iter()
        .map(|(c, cs)| {
            let VarConstAttrItem {
                attrs,
                vis,
                cnst,
                id,
                item_type,
                ..
            } = c;
            let (ret, def) = match item_type {
                ConstType::Value {
                    ty,
                    optional,
                    default,
                    ..
                } => {
                    let ret = if optional.is_some() {
                        quote! { ::core::option::Option<#ty> }
                    } else {
                        ty.into_token_stream()
                    };
                    let def = if optional.is_some() {
                        quote! { _ => ::core::option::Option::None, }
                    } else if let Some((_, def)) = default {
                        quote! { _ => #def, }
                    } else {
                        quote! {}
                    };
                    (ret, def)
                }
                ConstType::Flag { proxy } => {
                    if let Some((_, real, _, _)) = proxy {
                        // again, for r-a hover info.
                        other.extend(quote! {
                            const _: () = {
                                let _ = #item_name :: #real;
                            };
                        });
                        return TokenStream::new();
                    }
                    let ret = quote! { bool };
                    let def = quote! { _ => false };
                    (ret, def)
                }
            };

            quote! {
                #( #attrs )*
                #vis #cnst fn #id (&self) -> #ret {
                    match self {
                        #cs
                        #def
                    }
                }
            }
        })
        .collect::<TokenStream>();

    Ok(quote! {
        #item
        impl #item_name {
            #ci
        }
        #other
    })
}

struct VarConstAttrItem {
    attrs: Vec<Attribute>,
    vis: Visibility,
    cnst: Option<Token![const]>,
    id: Ident,
    item_type: ConstType,
}

impl Parse for VarConstAttrItem {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {
            attrs: Attribute::parse_outer(input)?,
            vis: input.parse()?,
            cnst: input.parse()?,
            id: input.parse()?,
            item_type: input.parse()?,
        })
    }
}

enum ConstType {
    Value {
        _colon: Token![:],
        ty: Type,
        optional: Option<Token![?]>,
        default: Option<(Token![=], Expr)>,
    },
    Flag {
        proxy: Option<(Token![for], Ident, Token![=], Expr)>,
    },
}

impl Parse for ConstType {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let l = input.lookahead1();
        if l.peek(Token![:]) {
            Ok(Self::Value {
                _colon: input.parse()?,
                ty: input.parse()?,
                optional: input.parse()?,
                default: {
                    let l = input.lookahead1();
                    if l.peek(Token![=]) {
                        Some((input.parse()?, input.parse()?))
                    } else {
                        None
                    }
                },
            })
        } else {
            Ok(Self::Flag {
                proxy: {
                    let l = input.lookahead1();
                    if l.peek(Token![for]) {
                        Some((
                            input.parse()?,
                            input.parse()?,
                            input.parse()?,
                            input.parse()?,
                        ))
                    } else {
                        None
                    }
                },
            })
        }
    }
}
