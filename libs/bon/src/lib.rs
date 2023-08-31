mod dotted;
mod perms;
mod scour;

pub mod prelude {
    pub use crate::dotted::{DottedPath, DottedPaths, JsonValue};
    pub use crate::perms::{find_acls, AclRule, Acls};
    pub use crate::perms::{Attempted, Denied, HasSecurityContext, Policy, SecurityContext};
    pub use crate::scour::Scoured; // TODO Remove this

    use serde::de::DeserializeOwned;
    use serde::ser::SerializeMap;
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

    #[derive(Clone, PartialEq, Debug)]
    pub struct TaggedJson(String, Json);

    impl TaggedJson {
        pub fn new(tag: String, value: Json) -> Self {
            Self(tag, value)
        }

        pub fn new_from(value: JsonValue) -> Result<Self, TaggedJsonError> {
            match value {
                JsonValue::Object(o) => {
                    let mut iter = o.into_iter();
                    if let Some(solo) = iter.next() {
                        if iter.next().is_some() {
                            Err(TaggedJsonError::Malformed)
                        } else {
                            Ok(Self(solo.0, solo.1.into()))
                        }
                    } else {
                        Err(TaggedJsonError::Malformed)
                    }
                }
                _ => Err(TaggedJsonError::Malformed),
            }
        }

        pub fn tag(&self) -> &str {
            &self.0
        }

        pub fn value(&self) -> &Json {
            &self.1
        }

        pub fn into_untagged(self) -> JsonValue {
            self.1.into()
        }

        pub fn into_tagged(self) -> JsonValue {
            self.into()
        }

        pub fn try_deserialize<T: DeserializeOwned>(self) -> Result<T, serde_json::Error> {
            serde_json::from_value(self.into_untagged())
        }
    }

    impl ToTaggedJson for TaggedJson {
        fn to_tagged_json(&self) -> Result<TaggedJson, TaggedJsonError> {
            Ok(self.clone())
        }
    }

    impl Serialize for TaggedJson {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            let mut map = serializer.serialize_map(Some(1))?;
            map.serialize_entry(&self.0, &self.1)?;
            map.end()
        }
    }

    impl<'de> Deserialize<'de> for TaggedJson {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            let value = JsonValue::deserialize(deserializer)?;

            TaggedJson::new_from(value)
                .map_err(|_| serde::de::Error::custom("Malformed tagged JSON"))
        }
    }

    impl TryFrom<JsonValue> for TaggedJson {
        type Error = TaggedJsonError;

        fn try_from(value: JsonValue) -> Result<Self, Self::Error> {
            TaggedJson::new_from(value)
        }
    }

    impl From<TaggedJson> for JsonValue {
        fn from(value: TaggedJson) -> Self {
            JsonValue::Object([(value.0, value.1.into())].into_iter().collect())
        }
    }

    #[derive(thiserror::Error, Debug)]
    pub enum TaggedJsonError {
        #[error("Malformed tagged JSON")]
        Malformed,
        #[error("JSON Error")]
        Json(#[source] serde_json::Error),
    }

    impl From<serde_json::Error> for TaggedJsonError {
        fn from(value: serde_json::Error) -> Self {
            Self::Json(value)
        }
    }

    pub trait HasTag {
        fn tag() -> std::borrow::Cow<'static, str>
        where
            Self: Sized;
    }

    pub trait ToTaggedJson {
        fn to_tagged_json(&self) -> Result<TaggedJson, TaggedJsonError>;
    }

    pub trait DeserializeTagged {
        fn from_tagged_json(tagged: &TaggedJson) -> Result<Option<Self>, serde_json::Error>
        where
            Self: Sized;
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
