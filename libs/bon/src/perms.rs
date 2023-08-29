use serde::{Deserialize, Serialize};

use crate::core::JsonValue;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AclRule {
    keys: Vec<String>,
    perm: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Acls {
    rules: Vec<AclRule>,
}

#[derive(Debug, PartialEq)]
pub struct AclProtection {
    pub path: String,
    pub acls: Acls,
}

impl AclProtection {
    pub fn prefix(self, value: &str) -> Self {
        Self {
            path: if self.path.is_empty() {
                value.to_owned()
            } else {
                format!("{}.{}", value, self.path)
            },
            acls: self.acls,
        }
    }
}

pub fn find_acls(value: &JsonValue) -> Option<Vec<AclProtection>> {
    match value {
        JsonValue::Null => None,
        JsonValue::Bool(_) => None,
        JsonValue::Number(_) => None,
        JsonValue::String(_) => None,
        JsonValue::Array(array) => Some(array.iter().flat_map(find_acls).flatten().collect()),
        JsonValue::Object(o) => {
            let acls = serde_json::from_value::<Acls>(value.clone());
            match acls {
                Ok(acls) => Some(vec![AclProtection {
                    path: "".to_owned(),
                    acls,
                }]),
                Err(_) => Some(
                    o.iter()
                        .flat_map(|(k, v)| find_acls(v).map(|o| o.into_iter().map(|p| p.prefix(k))))
                        .flatten()
                        .collect(),
                ),
            }
        }
    }
}

pub fn apply_read_acls(value: JsonValue) -> JsonValue {
    value
}

#[cfg(test)]
mod tests {
    use super::{find_acls, AclProtection, Acls, JsonValue};
    use serde_json::json;

    #[test]
    pub fn it_should_return_none_json_primitives() {
        assert!(find_acls(&JsonValue::Null).is_none());
        assert!(find_acls(&JsonValue::Bool(true)).is_none());
        assert!(find_acls(&JsonValue::Number(31337.into())).is_none());
        assert!(find_acls(&JsonValue::String("Hello".to_owned())).is_none());
    }

    #[test]
    pub fn it_should_return_some_json_array() {
        assert!(find_acls(&json!([])).is_some());
    }

    #[test]
    pub fn it_should_return_some_json_object() {
        assert!(find_acls(&json!({})).is_some());
    }

    #[test]
    pub fn it_should_return_basic_acl() {
        let acls = serde_json::to_value(Acls::default()).unwrap();
        assert_eq!(
            find_acls(&acls),
            Some(vec![AclProtection {
                path: "".to_owned(),
                acls: Acls::default()
            }])
        );
    }

    #[test]
    pub fn it_should_return_nested_acl_with_path() {
        let acls = Acls::default();
        let i = json!({
            "nested": {
                "acls": acls,
            }
        });
        assert_eq!(
            find_acls(&i),
            Some(vec![AclProtection {
                path: "nested.acls".to_owned(),
                acls: Acls::default()
            }])
        );
    }

    #[test]
    pub fn it_should_return_nested_arrays_with_path() {
        let acls = Acls::default();
        let i = json!({
            "scopes": [{
                "hello": {
                    "acls": acls,
                },
            }, {
                "world": {
                    "acls": acls,
                }
            }]
        });
        assert_eq!(
            find_acls(&i),
            Some(vec![
                AclProtection {
                    path: "scopes.hello.acls".to_owned(),
                    acls: Acls::default()
                },
                AclProtection {
                    path: "scopes.world.acls".to_owned(),
                    acls: Acls::default()
                }
            ])
        );
    }
}
