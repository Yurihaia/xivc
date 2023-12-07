use proc_macro2::Span;
use syn::{
    parse::{Parse, ParseStream},
    LitInt, LitStr, Token,
};

use std::{env, fs::File, io::Read, path::PathBuf};

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

// embed_data!("data_table.csv", row, enum TableRowEnum, u64)
// embed_data!("data_table.csv", row, u8, u64)
// wont be automatically rerun by RA when the sheets are changed unfortunately
pub fn embed_data(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let EmbedDataInput {
        file,
        row,
        row_type,
        output_type,
        ..
    } = syn::parse_macro_input!(item as EmbedDataInput);
    let mut path =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| String::from("./")));
    path.push(file.value());
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
