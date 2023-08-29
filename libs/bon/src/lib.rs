mod core {
    use std::str::FromStr;

    pub use serde_json::Value as JsonValue;

    #[derive(Default, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
    pub struct DottedPath(Vec<String>);

    impl DottedPath {
        pub fn join(&self, v: &str) -> Self {
            if self.0.is_empty() {
                Self(vec![v.to_owned()])
            } else {
                Self(
                    self.0
                        .clone()
                        .into_iter()
                        .chain(std::iter::once(v.to_owned()))
                        .collect(),
                )
            }
        }

        pub fn prefix(&self, v: &str) -> Self {
            if self.0.is_empty() {
                Self(vec![v.to_owned()])
            } else {
                Self(
                    std::iter::once(v.to_owned())
                        .chain(self.0.clone().into_iter())
                        .collect(),
                )
            }
        }
    }

    impl From<Vec<&str>> for DottedPath {
        fn from(value: Vec<&str>) -> Self {
            Self(value.into_iter().map(|v| v.to_owned()).collect())
        }
    }

    impl From<&str> for DottedPath {
        fn from(value: &str) -> Self {
            value.split(".").collect::<Vec<_>>().into()
        }
    }

    impl ToString for DottedPath {
        fn to_string(&self) -> String {
            self.0.join(".")
        }
    }

    impl FromStr for DottedPath {
        type Err = ();

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            Ok(s.into())
        }
    }
}

mod scour {
    use serde::de::DeserializeOwned;

    pub use crate::core::{DottedPath, JsonValue};

    #[derive(Debug, PartialEq)]
    pub struct Scoured<T> {
        pub path: DottedPath,
        pub value: T,
    }

    impl<T> Scoured<T> {
        pub fn prefix(self, value: &str) -> Self {
            Self {
                path: self.path.prefix(value),
                value: self.value,
            }
        }
    }

    pub(crate) fn scour<T>(value: &JsonValue) -> Option<Vec<Scoured<T>>>
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
}

mod perms;

pub mod prelude {
    pub use crate::core::JsonValue;
    pub use crate::perms::{apply_read_acls, find_acls, AclRule, Acls};
}
