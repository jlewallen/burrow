use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(ToTaggedJson)]
pub fn json_derive_to_tagged_json(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let done = quote! {
        impl ToTaggedJson for #name {
            fn to_tagged_json(&self) -> std::result::Result<TaggedJson, TaggedJsonError> {
                let value = serde_json::to_value(self)?;
                let key = stringify!(#name);
                let mut c = key.chars();
                let key = match c.next() {
                    None => String::new(),
                    Some(f) => f.to_lowercase().collect::<String>() + c.as_str(),
                };
                Ok(TaggedJson::new(key, value))
            }
        }
    };

    done.into()
}

#[proc_macro_attribute]
pub fn action(_metadata: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as DeriveInput);
    let done = quote! {
        #[derive(Debug, Serialize, Deserialize, ToTaggedJson)]
        #item
    };

    done.into()
}
