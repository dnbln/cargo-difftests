use proc_macro2::TokenStream;
use quote::{format_ident, quote};

#[proc_macro_attribute]
pub fn test(
    attr: proc_macro::TokenStream,
    body: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let setup_fn = if attr.is_empty() {
        syn::parse_quote!(setup_difftests)
    } else {
        syn::parse_macro_input!(attr as syn::Path)
    };
    let test_fn = syn::parse_macro_input!(body as syn::ItemFn);
    derive_test(setup_fn, test_fn)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

fn derive_test(setup_fn: syn::Path, mut test_fn: syn::ItemFn) -> syn::Result<TokenStream> {
    let inner_test_fn_name = format_ident!("__difftests_test");
    let test_name = std::mem::replace(&mut test_fn.sig.ident, inner_test_fn_name.clone());
    if !test_fn.sig.inputs.is_empty() {
        return Err(syn::Error::new_spanned(
            test_fn.sig.inputs,
            "test functions cannot take arguments",
        ));
    }

    test_fn
        .sig
        .inputs
        .push(syn::parse_quote!(difftests_env: DifftestsEnv));

    let test_name_str = test_name.to_string();

    let test_fn = quote! {
        #[test]
        fn #test_name() {
            let difftests_env = #setup_fn(#test_name_str);
            #[cfg_attr(cargo_difftests, inline(never))]
            fn difftests_guard() {}

            let _g = difftests_guard();
            #test_fn

            #inner_test_fn_name(difftests_env)
        }
    };

    Ok(test_fn)
}
