mod dotted;
mod perms;
mod scour;
mod tagged;

pub mod prelude {
    pub use crate::dotted::{DottedPath, DottedPaths, JsonValue};
    pub use crate::perms::{find_acls, AclRule, Acls};
    pub use crate::perms::{Attempted, Denied, HasSecurityContext, Policy, SecurityContext};
    pub use crate::scour::Scoured; // TODO Remove this
    pub use crate::tagged::{DeserializeTagged, HasTag, TaggedJson, TaggedJsonError, ToTaggedJson};

    use serde::{Deserialize, Serialize};

    #[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
    pub struct Json(pub JsonValue);

    impl Json {
        pub fn inner(&self) -> &JsonValue {
            &self.0
        }

        pub fn into_inner(self) -> JsonValue {
            self.0
        }

        pub fn tagged(&self, tag: &str) -> Option<TaggedJson> {
            match &self.0 {
                JsonValue::Object(object) => object
                    .get(tag)
                    .map(|v| TaggedJson::new(tag.to_owned(), v.clone().into())),
                _ => None,
            }
        }
    }

    impl From<JsonValue> for Json {
        fn from(value: JsonValue) -> Self {
            Self(value)
        }
    }

    impl From<Json> for JsonValue {
        fn from(value: Json) -> Self {
            value.0
        }
    }

    use std::borrow::Cow;

    pub fn identifier_to_key(id: &'static str) -> Cow<'static, str> {
        let mut c = id.chars();
        match c.next() {
            Some(f) => f.to_lowercase().collect::<String>() + c.as_str(),
            None => panic!("Empty key in tagged JSON."),
        }
        .into()
    }
}
