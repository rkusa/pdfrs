#[allow(unused_extern_crates)]
extern crate proc_macro;

use proc_macro::TokenStream;
use quote::quote;

fn convert(mut input: syn::ItemFn, args: syn::AttributeArgs) -> Result<TokenStream, syn::Error> {
    let sig = &mut input.sig;
    let body = &input.block;
    let attrs = &input.attrs;
    let vis = input.vis;

    sig.inputs = syn::punctuated::Punctuated::new();

    let path = match args.first() {
        Some(syn::NestedMeta::Lit(lit)) => match lit {
            syn::Lit::Str(path) => path.value(),
            other => {
                return Err(syn::Error::new_spanned(
                    other,
                    "Unsupported attribute type inside the macro",
                ))
            }
        },
        other => {
            return Err(syn::Error::new_spanned(
                other,
                "Unsupported attribute inside the macro",
            ));
        }
    };

    let result = quote! {
        #[async_std::test]
        #(#attrs)*
        #vis #sig {
            use std::io::Write;
            use chrono::{Utc, TimeZone};

            // Source: https://github.com/colin-kiegel/rust-pretty-assertions/issues/24
            #[derive(PartialEq, Eq)]
            struct PrettyString<'a>(&'a str);
            impl<'a> std::fmt::Debug for PrettyString<'a> {
                fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                    f.write_str(self.0)
                }
            }

            let mut path = std::path::PathBuf::from("./tests/");
            path.push(#path);
            path.set_extension("result.pdf");

            let mut result = Vec::new();
            let mut doc = Document::new(&mut result).await.unwrap();
            doc.set_id("test");
            doc.set_creation_date(Utc.ymd(2019, 6, 2).and_hms(14, 28, 0));
            doc.set_producer("pdfrs [test] (github.com/rkusa/pdfrs)");

            {
                let doc = &mut doc;
                #body
            }

            doc.end().await.unwrap();

            let mut file = File::create(path).expect("Error creating result file");
            file.write_all(&result).expect("Error writing result to file");

            let expected = include_bytes!(#path);
            pretty_assertions::assert_eq!(
                PrettyString(&String::from_utf8_lossy(&result)),
                PrettyString(&String::from_utf8_lossy(expected)),
                "Resulting PDF does not match expected one"
            );
        }
    };

    Ok(result.into())
}

#[proc_macro_attribute]
pub fn test(args: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let args = syn::parse_macro_input!(args as syn::AttributeArgs);

    for attr in &input.attrs {
        if attr.path.is_ident("test") {
            let msg = "second test attribute is supplied";
            return syn::Error::new_spanned(&attr, msg)
                .to_compile_error()
                .into();
        }
    }

    if input.sig.inputs.len() != 1 {
        let msg = "the test function must accept a document as its only argument";
        return syn::Error::new_spanned(&input.sig.inputs, msg)
            .to_compile_error()
            .into();
    }

    convert(input, args).unwrap_or_else(|e| e.to_compile_error().into())
}
