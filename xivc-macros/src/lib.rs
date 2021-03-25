extern crate proc_macro;
use proc_macro2::Span;
use syn::{
    parse::{Parse, ParseStream},
    LitInt, LitStr, Token,
};

use std::{fs::File, io::Read, path::PathBuf};

use quote::quote;

struct EmbedDataInput {
    file: LitStr,
    _c1: Token![,],
    row: syn::Expr,
    _c2: Token![,],
    row_type: RowType,
    _c3: Token![,],
    output_type: syn::Type,
}

enum RowType {
    Range(syn::Type),
    Enum(syn::Path),
}

impl Parse for EmbedDataInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(EmbedDataInput {
            file: input.parse()?,
            _c1: input.parse()?,
            row: input.parse()?,
            _c2: input.parse()?,
            row_type: input.parse()?,
            _c3: input.parse()?,
            output_type: input.parse()?,
        })
    }
}

impl Parse for RowType {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(Token![enum]) {
            input.parse::<Token![enum]>()?;
            Ok(RowType::Enum(input.parse()?))
        } else {
            Ok(RowType::Range(input.parse()?))
        }
    }
}

struct DataTable<'c> {
    rows: Vec<(&'c str, Vec<&'c str>)>,
}

#[proc_macro]
// embed_data!("data_table.csv", row, enum TableRowEnum, u64)
// embed_data!("data_table.csv", row, u8, u64)
pub fn embed_data(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let EmbedDataInput {
        file,
        row,
        row_type,
        output_type,
        ..
    } = syn::parse_macro_input!(item as EmbedDataInput);
    let path = PathBuf::from(file.value());
    let mut csv_str = String::new();
    File::open(&path)
        .unwrap()
        .read_to_string(&mut csv_str)
        .unwrap();
    // Process CSV
    let mut csv = DataTable { rows: Vec::new() };
    for x in csv_str.lines() {
        if !x.is_empty() {
            let mut iter = x.split(',').map(|x| x.trim()).filter(|x| !x.is_empty());
            let row = iter.next().unwrap();
            let content = iter.collect::<Vec<_>>();
            csv.rows.push((row, content));
        }
    }
    //
    match row_type {
        RowType::Enum(v) => {
            let rows = csv.rows.iter().map(|(n, vals)| {
                let name = quote::format_ident!("{}", n);
                let vals = vals.iter().map(|x| LitInt::new(x, Span::call_site()));
                quote! {
                    #v::#name => Some((#(#vals as #output_type),*))
                }
            });
            quote! {
                match #row {
                    #(
                        #rows
                    ),*
                }
            }
        }
        // Unused but I still want to keep it in I guess
        RowType::Range(_ty) => {
            let rows = csv.rows.iter().map(|(n, vals)| {
                let name = LitInt::new(n, Span::call_site());
                let vals = vals.iter().map(|x| LitInt::new(x, Span::call_site()));
                quote! {
                    #name => Some((#(#vals as #output_type),*))
                }
            });
            quote! {
                // Check to make sure the type matches
                match #row {
                    #(
                        #rows
                    ),*,
                    _ => None,
                }
            }
        }
    }
    .into()
}

/*
const ATTR_XIVC_FLAG: &[&str] = &["xivc", "flag"];
const ATTR_XIVC_COOLDOWN: &[&str] = &["xivc", "cooldown"];
// const ATTR_XIVC_COMBO: &[&str] = &["xivc", "combo"];
const ATTR_XIVC_PARSE: &[&str] = &["xivc", "parse"];

#[proc_macro_attribute]
pub fn action(
    _: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let mut ac = parse_macro_input!(item as syn::ItemEnum);
    let enum_name = &ac.ident;

    let mut errs = vec![];

    let mut add_parse = false;
    let mut cooldown_name = None;
    ac.attrs.retain(|attr| {
        let m = match attr.parse_meta() {
            Ok(v) => v,
            Err(_) => return true,
        };
        if is_path(m.path(), ATTR_XIVC_PARSE) {
            add_parse = true;
        } else if is_path(m.path(), ATTR_XIVC_COOLDOWN) {
            match action_attr::XivcCooldownEnum::from_meta(&m) {
                Ok(v) => cooldown_name = Some(v.name),
                Err(e) => errs.push(e),
            }
        } else {
            return true;
        }
        false
    });
    // flag => set variants[]
    let mut flagfns: HashMap<Ident, Vec<Ident>> = HashMap::new();

    // a list of
    // Self::Variant => Some(Cooldown::Variant)
    let mut self_to_cd = TokenStream::new();
    let mut cd_to_self = TokenStream::new();
    let mut cd_vars = TokenStream::new();

    let mut parse_impl = TokenStream::new();

    for v in ac.variants.iter_mut() {
        let var_id = &v.ident;
        if add_parse {
            let name = var_id.to_string();
            parse_impl.extend(quote!(
                #name => Ok(Self::#var_id),
            ));
        }
        v.attrs.retain(|attr| {
            let m = match attr.parse_meta() {
                Ok(v) => v,
                Err(_) => return true,
            };
            if is_path(m.path(), ATTR_XIVC_FLAG) {
                match action_attr::XivcFlagVar::from_meta(&m) {
                    Ok(v) => {
                        for x in v.0.iter() {
                            match x.get_ident() {
                                Some(id) => {
                                    flagfns.entry(id.clone()).or_default().push(var_id.clone());
                                }
                                None => errs.push(darling::Error::custom(
                                    "Expected an ident, found a path",
                                )),
                            }
                        }
                    }
                    Err(e) => errs.push(e),
                }
            } else if is_path(m.path(), ATTR_XIVC_COOLDOWN) {
                match (action_attr::XivcCooldownVar::from_meta(&m), &cooldown_name) {
                    (Ok(_), Some(cd_name)) => {
                        self_to_cd.extend(quote!(
                            Self::#var_id => Some(#cd_name::#var_id),
                        ));
                        cd_to_self.extend(quote!(
                            #cd_name::#var_id => Self::#var_id,
                        ));
                        cd_vars.extend(quote!(
                            #var_id,
                        ));
                    }
                    (Err(e), _) => errs.push(e),
                    (_, None) => errs.push(darling::Error::custom(
                        "No cooldown name attribute on the enum declaration.",
                    )),
                }
            } else {
                return true;
            }
            false
        });
    }

    let parse_impl = if add_parse {
        quote!(
            impl ::std::str::FromStr for #enum_name {
                type Err = ();
                fn from_str(s: &str) -> Result<Self, ()> {
                    match s {
                        #parse_impl
                        _ => Err(())
                    }
                }
            }
        )
    } else {
        parse_impl
    };

    let cooldown_impl = if let Some(cd_name) = cooldown_name {
        quote!(
            pub enum #cd_name {
                #cd_vars
            }

            impl ::std::convet::From<#cd_name> for #enum_name {
                fn from()
            }
        )
    } else {
        TokenStream::new()
    };

    quote!(
        #ac



        impl #enum_name {

        }

        #parse_impl
    )
    .into()
}

fn is_path(path: &Path, val: &[&str]) -> bool {
    path.leading_colon.is_none() && path.segments.iter().enumerate().all(|(i, v)| {
        matches!(v, PathSegment { ident, arguments: PathArguments::None } if ident == val[i])
    }) && !path.segments.trailing_punct()
}

mod action_attr {
    #![allow(clippy::large_enum_variant)]
    use std::collections::HashMap;

    use darling::{util::PathList, FromMeta};
    use proc_macro2::{Span, TokenStream};
    use syn::{
        parenthesized,
        parse::{Parse, ParseStream},
        punctuated::Punctuated,
        token::Paren,
        Ident, Lit, Path, Token, Visibility,
    };

    #[derive(FromMeta)]
    pub struct XivcFlagVar(pub PathList);

    #[derive(FromMeta)]
    pub struct XivcParseEnum {
        pub case_sensitive: Option<()>,
    }

    #[derive(FromMeta)]
    pub struct XivcCooldownVar;

    #[derive(FromMeta)]
    pub struct XivcCooldownEnum {
        pub name: Ident,
    }
}

// is this too verbose?????
// of course it is but of course the alternative
// is cursed enum casting or just boilerplate

/*
#[xivc::parse]
#[xivc::cooldown(GnbActionCooldown)]
pub enum GnbAction {
    #[xivc::flag(gcd)]
    Keen,
    #[xivc::flag(gcd)]
    #[xivc::combo(KeenCombo, Keen)]
    Brutal,
    #[xivc::flag(gcd)]
    #[xivc::combo(KeenCombo, Brutal)]
    Solid,
    #[xivc::flag(gcd)]
    #[xivc::cooldown(3000, speed_scale = sks)]
    Gnashing,
    #[xivc::flag(gcd)]
    #[xivc::cooldown(6000, speed_scale = sks)]
    Sonic,
    #[xivc::cooldown(3000, charge = 2)]
    Divide,
    #[xivc::cooldown(3000)]
    Blasting,
    #[xivc::cooldown(6000)]
    NoMercy,
}
*/
*/
