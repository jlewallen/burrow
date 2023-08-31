mod compare;
mod dotted;
mod perms;
mod scour;
mod tagged;
#[cfg(test)]
mod tests;

pub mod prelude {
    pub use crate::compare::{
        AnyChanges, CompareChanges, CompareError, Modified, Original, TreeDiff,
    };
    pub use crate::dotted::{DottedPath, DottedPaths, JsonValue};
    pub use crate::perms::{find_acls, AclRule, Acls};
    pub use crate::perms::{Attempted, Denied, HasSecurityContext, Policy, SecurityContext};
    pub use crate::scour::{scour, Scoured};
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

    #[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct JsonTemplate(JsonValue);

    pub const JSON_TEMPLATE_VALUE_SENTINEL: &str = "!#$value";

    impl JsonTemplate {
        pub fn instantiate(self, value: &JsonValue) -> JsonValue {
            match self.0 {
                JsonValue::Null | JsonValue::Bool(_) | JsonValue::Number(_) => self.0,
                JsonValue::String(s) => {
                    if s == JSON_TEMPLATE_VALUE_SENTINEL {
                        value.clone()
                    } else {
                        JsonValue::String(s)
                    }
                }
                JsonValue::Array(v) => JsonValue::Array(
                    v.into_iter()
                        .map(|c| JsonTemplate(c).instantiate(value))
                        .collect(),
                ),
                JsonValue::Object(v) => JsonValue::Object(
                    v.into_iter()
                        .map(|(k, v)| (k, JsonTemplate(v).instantiate(value)))
                        .collect(),
                ),
            }
        }
    }

    impl HasTag for JsonTemplate {
        fn tag() -> std::borrow::Cow<'static, str>
        where
            Self: Sized,
        {
            "jsonTemplate".into()
        }
    }

    impl ToTaggedJson for JsonTemplate {
        fn to_tagged_json(&self) -> Result<TaggedJson, TaggedJsonError> {
            let value = serde_json::to_value(self)?;
            Ok(TaggedJson::new(Self::tag().to_string(), value.into()))
        }
    }

    impl From<JsonValue> for JsonTemplate {
        fn from(value: JsonValue) -> Self {
            Self(value)
        }
    }

    impl From<TaggedJson> for JsonTemplate {
        fn from(value: TaggedJson) -> Self {
            Self(value.into_tagged())
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
