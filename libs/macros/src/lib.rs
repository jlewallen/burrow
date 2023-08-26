use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Reply)]
pub fn derive_reply(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let done = quote! {
        impl Reply for #name {}
    };

    done.into()
}

#[proc_macro_derive(ToTaggedJson)]
pub fn derive_to_tagged_json(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let done = quote! {
        impl HasTag for #name {
            fn tag() -> std::borrow::Cow<'static, str>
            where
                Self: Sized,
            {
                identifier_to_key(stringify!(#name))
            }
        }

        impl ToTaggedJson for #name {
            fn to_tagged_json(&self) -> std::result::Result<TaggedJson, TaggedJsonError> {
                let tag = identifier_to_key(stringify!(#name));
                let value = serde_json::to_value(self)?;
                Ok(TaggedJson::new(tag.to_string(), value.into()))
            }
        }
    };

    done.into()
}

#[proc_macro_derive(DeserializeTagged)]
pub fn derive_deserialize_tagged(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let done = quote! {
        impl DeserializeTagged for #name {
            fn from_tagged_json(tagged: &TaggedJson) -> Result<Option<Self>, serde_json::Error> {
                let tag = identifier_to_key(stringify!(#name));
                if tag != tagged.tag() {
                    return Ok(None);
                }

                Ok(Some(tagged.clone().try_deserialize()?))
            }
        }
    };

    done.into()
}

#[proc_macro_derive(HasActionSchema)]
pub fn derive_has_action_schema(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;

    let done = quote! {
        impl HasActionSchema for #name {
            fn action_schema(schema: ActionSchema) -> ActionSchema {
                schema
            }
        }
    };

    done.into()
}

#[proc_macro_attribute]
pub fn action(_metadata: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as DeriveInput);
    let done = quote! {
        #[derive(Debug, Serialize, Deserialize, ToTaggedJson, DeserializeTagged, HasActionSchema)]
        #item
    };

    done.into()
}
