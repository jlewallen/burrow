use serde::de::DeserializeOwned;

pub use crate::dotted::{DottedPath, JsonValue};

#[derive(Clone, PartialEq, Debug)]
pub struct Scoured<T> {
    pub path: DottedPath,
    pub value: T,
}

impl<T> Scoured<T> {
    pub fn new(path: DottedPath, value: T) -> Self {
        Self { path, value }
    }

    pub fn prefix(self, value: &str) -> Self {
        Self {
            path: self.path.prefix(value),
            value: self.value,
        }
    }

    pub fn into(self) -> T {
        self.value
    }

    pub fn value(&self) -> &T {
        &self.value
    }
}

pub fn scour<T>(value: &JsonValue) -> Option<Vec<Scoured<T>>>
where
    T: DeserializeOwned,
{
    match value {
        JsonValue::Object(o) => {
            let value = serde_json::from_value::<T>(value.clone());
            match value {
                Ok(value) => Some(vec![Scoured {
                    path: DottedPath::default(),
                    value,
                }]),
                Err(_) => Some(
                    o.iter()
                        .flat_map(|(k, v)| scour(v).map(|o| o.into_iter().map(|p| p.prefix(k))))
                        .flatten()
                        .collect(),
                ),
            }
        }
        JsonValue::Array(array) => Some(array.iter().flat_map(scour).flatten().collect()),
        JsonValue::String(_) => None,
        JsonValue::Number(_) => None,
        JsonValue::Bool(_) => None,
        JsonValue::Null => None,
    }
}
