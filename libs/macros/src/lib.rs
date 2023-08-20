use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Reply)]
pub fn json_derive_reply(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let done = quote! {
        impl Reply for #name {}
    };

    done.into()
}

#[proc_macro_derive(ToTaggedJson)]
pub fn json_derive_to_tagged_json(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let done = quote! {
        impl ToTaggedJson for #name {
            fn to_tagged_json(&self) -> std::result::Result<TaggedJson, TaggedJsonError> {
                let tag = identifier_to_key(stringify!(#name));
                let value = serde_json::to_value(self)?;
                Ok(TaggedJson::new(tag, value.into()))
            }
        }
    };

    done.into()
}

#[proc_macro_derive(DeserializeTagged)]
pub fn json_derive_deserialize_tagged(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let done = quote! {
        impl DeserializeTagged for #name {
            fn from_tagged_json(tagged: TaggedJson) -> Result<Option<Self>, serde_json::Error> {
                let tag = identifier_to_key(stringify!(#name));
                if tag != tagged.tag() {
                    return Ok(None);
                }

                Ok(Some(tagged.try_deserialize()?))
            }
        }
    };

    done.into()
}

#[proc_macro_attribute]
pub fn action(_metadata: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as DeriveInput);
    let done = quote! {
        #[derive(Debug, Serialize, Deserialize, ToTaggedJson, DeserializeTagged)]
        #item
    };

    done.into()
}
