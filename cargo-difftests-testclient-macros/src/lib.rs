/*
 *        Copyright (c) 2023-2024 Dinu Blanovschi
 *
 *    Licensed under the Apache License, Version 2.0 (the "License");
 *    you may not use this file except in compliance with the License.
 *    You may obtain a copy of the License at
 *
 *        https://www.apache.org/licenses/LICENSE-2.0
 *
 *    Unless required by applicable law or agreed to in writing, software
 *    distributed under the License is distributed on an "AS IS" BASIS,
 *    WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 *    See the License for the specific language governing permissions and
 *    limitations under the License.
 */

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
    derive_test(setup_fn, test_fn, true)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

#[proc_macro_attribute]
pub fn wrap_test(
    attr: proc_macro::TokenStream,
    body: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let setup_fn = if attr.is_empty() {
        syn::parse_quote!(setup_difftests)
    } else {
        syn::parse_macro_input!(attr as syn::Path)
    };
    let test_fn = syn::parse_macro_input!(body as syn::ItemFn);
    derive_test(setup_fn, test_fn, false)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

fn derive_test(setup_fn: syn::Path, mut test_fn: syn::ItemFn, include_test_attr: bool) -> syn::Result<TokenStream> {
    let inner_test_fn_name = format_ident!("__difftests_test");
    let test_name = std::mem::replace(&mut test_fn.sig.ident, inner_test_fn_name.clone());
    if test_fn.sig.inputs.len() != 1 {
        return Err(syn::Error::new_spanned(
            test_fn.sig.inputs,
            "test function must take the DifftestsEnv as an argument",
        ));
    }

    let test_name_str = test_name.to_string();

    let tattr = if include_test_attr {
        quote! { #[test] }
    } else {
        quote! {}
    };

    let test_fn = quote! {
        #tattr
        fn #test_name() {
            let difftests_env = #setup_fn(#test_name_str);
            #[cfg_attr(cargo_difftests, inline(never))]
            fn difftests_guard() {}

            let _g = difftests_guard();
            #test_fn

            #inner_test_fn_name(&difftests_env)
        }
    };

    Ok(test_fn)
}
