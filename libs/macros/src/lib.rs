use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(ToJson)]
pub fn json_derive_to_json(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let done = match input.data {
        _ => {
            quote! {
                impl ToJson for #name {
                    fn to_json(&self) -> std::result::Result<serde_json::Value, serde_json::Error> {
                        let value = serde_json::to_value(self)?;
                        let key = stringify!(#name);
                        Ok(serde_json::json!({ key: value }))
                    }
                }
            }
        }
    };

    done.into()
}

#[proc_macro_attribute]
pub fn action(_metadata: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as DeriveInput);
    let done = quote! {
        #[derive(Debug, Serialize, Deserialize, ToJson)]
        #item
    };

    done.into()
}
