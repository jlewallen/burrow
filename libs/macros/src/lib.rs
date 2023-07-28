use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(ToJson)]
pub fn json_derive_to_json(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let done = match input.data {
        syn::Data::Struct(_) => {
            quote! {
                impl ToJson for #name {
                    fn to_json(&self) -> std::result::Result<serde_json::Value, serde_json::Error> {
                        Ok(serde_json::to_value(self)?)
                    }
                }
            }
        }
        _ => unimplemented!(),
    };

    done.into()
}

#[proc_macro_attribute]
pub fn action(_metadata: TokenStream, item: TokenStream) -> TokenStream {
    let item: proc_macro2::TokenStream = item.into();
    let done = quote! {
        #[derive(Debug, Serialize, Deserialize, ToJson)]
        #item
    };

    done.into()
}
