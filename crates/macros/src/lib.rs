mod csv;
mod vc;

extern crate proc_macro;

#[proc_macro]
pub fn embed_data(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    csv::embed_data(item)
}

#[proc_macro_attribute]
pub fn var_consts(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    vc::var_const_impl(attr.into(), item.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
