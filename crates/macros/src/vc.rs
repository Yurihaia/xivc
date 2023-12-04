use std::collections::HashMap;

use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, ToTokens, TokenStreamExt};
use syn::{
    parse::{Parse, ParseStream},
    spanned::Spanned,
    Attribute, Expr, Fields, ItemEnum, Meta, Path, Token, Type, Visibility,
};

pub fn var_const_impl(item: TokenStream) -> syn::Result<TokenStream> {
    let mut en = syn::parse2::<ItemEnum>(item)?;
    let en_name = &en.ident;

    // let consts = Vec::new();

    let mut kept_attrs = Vec::with_capacity(en.attrs.capacity());

    let mut args: HashMap<Ident, (Span, ArgMeta)> = HashMap::new();

    for x in en.attrs.drain(..) {
        let path = x.path();
        let span = x.meta.span();
        if path.is_ident("property") {
            let meta: PropArgMeta = x.parse_args()?;
            if args.contains_key(&meta.id) {
                return Err(syn::Error::new(
                    meta.id.span(),
                    format_args!("duplicate argument {}", meta.id),
                ));
            }
            args.insert(meta.id.clone(), (span, ArgMeta::Const(Vec::new(), meta)));
        } else if path.is_ident("flag") {
            let meta: FlagArgMeta = x.parse_args()?;
            if args.contains_key(&meta.id) {
                return Err(syn::Error::new(
                    meta.id.span(),
                    format_args!("duplicate argument {}", meta.id),
                ));
            }
            args.insert(meta.id.clone(), (span, ArgMeta::Flag(Vec::new(), meta)));
        } else {
            kept_attrs.push(x);
        }
    }

    en.attrs = kept_attrs;

    for x in &mut en.variants {
        let mut kept_attrs = Vec::with_capacity(x.attrs.capacity());

        for a in x.attrs.drain(..) {
            let span = a.span();
            if let Some((_, meta)) = get_path_ident(a.path()).and_then(|x| args.get_mut(&x)) {
                match a.meta {
                    Meta::Path(_) => match meta {
                        ArgMeta::Flag(vars, _) => {
                            vars.push((x.ident.clone(), EnumSkipType::from_fields(&x.fields)));
                            continue;
                        }
                        _ => return Err(syn::Error::new(span, "a path attribute must be a flag")),
                    },
                    Meta::NameValue(nv) => match meta {
                        ArgMeta::Const(vars, ..) => {
                            vars.push((
                                x.ident.clone(),
                                nv.value,
                                EnumSkipType::from_fields(&x.fields),
                            ));
                            continue;
                        }
                        _ => {
                            return Err(syn::Error::new(
                                span,
                                "a name-value attribute must have a value",
                            ))
                        }
                    },
                    _ => {}
                }
            }
            kept_attrs.push(a);
        }

        x.attrs = kept_attrs;
    }

    let arg_iter = args.into_iter().map(|(id, (_, meta))| match meta {
        ArgMeta::Const(vars, meta) => {
            let viter = vars.into_iter().map(|(var, val, skip)| {
                quote! {
                    Self::#var #skip => #val,
                }
            });
            let def = meta
                .default
                .map(|ArgDefault { val, .. }| {
                    quote! {
                        _ => #val,
                    }
                })
                .unwrap_or_else(|| quote! {});
            let c = meta.c;
            let vis = meta.vis;
            let ty = meta.ty;
            let id = meta.id;
            let attrs = meta.attrs.into_iter();
            quote! {
                #( #attrs )*
                #vis #c fn #id (&self) -> #ty {
                    match self {
                        #( #viter )*
                        #def
                    }
                }
            }
        }
        ArgMeta::Flag(vars, meta) => {
            let it = vars.iter().map(|(id, skip)| quote! { #id #skip });
            let tks = if vars.is_empty() {
                quote! {}
            } else {
                quote! { #( Self::#it )|* => true, }
            };
            let vis = meta.vis;
            let attrs = meta.attrs.into_iter();
            quote! {
                #( #attrs )*
                #vis const fn #id (&self) -> bool {
                    match self {
                        #tks
                        _ => false,
                    }
                }
            }
        }
    });

    Ok(quote! {
        #en

        impl #en_name {
            #( #arg_iter )*
        }
    })
}

fn get_path_ident(path: &Path) -> Option<Ident> {
    if let Some(v) = path.get_ident() {
        return Some(v.clone());
    }

    if path.segments.len() == 2 {
        let seg0 = &path.segments[0];
        let seg1 = &path.segments[1];
        if seg0.arguments.is_none() && seg0.ident == "var_const" && seg1.arguments.is_none() {
            Some(seg1.ident.clone())
        } else {
            None
        }
    } else {
        None
    }
}

enum ArgMeta {
    Const(Vec<(Ident, Expr, EnumSkipType)>, PropArgMeta),
    Flag(Vec<(Ident, EnumSkipType)>, FlagArgMeta),
}

enum EnumSkipType {
    None,
    Paren,
    Curly,
}

impl EnumSkipType {
    pub fn from_fields(fields: &Fields) -> Self {
        match fields {
            Fields::Named(_) => Self::Curly,
            Fields::Unit => Self::None,
            Fields::Unnamed(_) => Self::Paren,
        }
    }
}

impl ToTokens for EnumSkipType {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        match self {
            Self::None => (),
            Self::Curly => tokens.append_all(quote! { { .. } }),
            Self::Paren => tokens.append_all(quote! { (..) }),
        }
    }
}

struct PropArgMeta {
    attrs: Vec<Attribute>,
    vis: Visibility,
    c: Option<Token![const]>,
    id: Ident,
    _colon: Token![:],
    ty: Type,
    default: Option<ArgDefault>,
}

impl Parse for PropArgMeta {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {
            attrs: input.call(Attribute::parse_outer)?,
            vis: input.parse()?,
            c: {
                let l = input.lookahead1();
                if l.peek(Token![const]) {
                    Some(input.parse()?)
                } else {
                    None
                }
            },
            id: input.parse()?,
            _colon: input.parse()?,
            ty: input.parse()?,
            default: {
                let l = input.lookahead1();
                if l.peek(Token![=]) {
                    Some(input.parse()?)
                } else {
                    None
                }
            },
        })
    }
}

struct ArgDefault {
    _eq: Token![=],
    val: Expr,
}

impl Parse for ArgDefault {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {
            _eq: input.parse()?,
            val: input.parse()?,
        })
    }
}

struct FlagArgMeta {
    attrs: Vec<Attribute>,
    vis: Visibility,
    id: Ident,
}

impl Parse for FlagArgMeta {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {
            attrs: input.call(Attribute::parse_outer)?,
            vis: input.parse()?,
            id: input.parse()?,
        })
    }
}

// struct IndexArgMeta {
//     attrs: Vec<Attribute>,
//     vis: Visibility,
//     id: Ident,
// }

// use proc_macro2::Ident;
// use syn::{
//     parse::{Parse, ParseStream},
//     Attribute, Expr, Token, Type, Visibility,
// };

// struct PropertyDeclMeta {
//     pub attrs: Vec<Attribute>,
//     pub vis: Visibility,
//     pub const_token: Option<Token![const]>,
//     pub ident: Ident,
//     pub colon_token: Token![:],
//     pub ty: Type,
//     pub eq_token: Option<Token![=]>,
//     pub default: Option<Expr>,
// }

// impl Parse for PropertyDeclMeta {
//     fn parse(input: ParseStream) -> syn::Result<Self> {
//         let attrs = input.call(Attribute::parse_outer)?;
//         let vis = input.parse()?;
//         let const_token = input.parse()?;
//         let ident = input.parse()?;
//         let colon_token = input.parse()?;
//         let ty = input.parse()?;
//         let eq_token = input.parse()?;
//         let default = if let Some(_) = eq_token {
//             Some(input.parse()?)
//         } else {
//             None
//         };
//         Ok(Self {
//             attrs,
//             vis,
//             const_token,
//             ident,
//             colon_token,
//             ty,
//             eq_token,
//             default,
//         })
//     }
// }

// struct FlagDeclMeta {
//     pub attrs: Vec<Attribute>,
//     pub vis: Visibility,
//     pub ident: Ident,
// }

// impl Parse for FlagDeclMeta {
//     fn parse(input: ParseStream) -> syn::Result<Self> {
//         Ok(Self {
//             attrs: input.call(Attribute::parse_outer)?,
//             vis: input.parse()?,
//             ident: input.parse()?,
//         })
//     }
// }
