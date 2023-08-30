use serde::{Deserialize, Serialize};

use crate::{core::JsonValue, scour::Scoured};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Subject {
    Everybody,
    Owner,
    Creator,
    Key(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Perm {
    Read,
    Write,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AclRule {
    keys: Vec<Subject>,
    perm: Perm,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Acls {
    #[serde(default)]
    rules: Vec<AclRule>,
}

#[derive(Deserialize)]
pub struct TaggedAcls {
    #[allow(dead_code)]
    acls: Acls,
}

impl Into<Acls> for TaggedAcls {
    fn into(self) -> Acls {
        self.acls
    }
}

pub fn find_acls(value: &JsonValue) -> Option<Vec<Scoured<Acls>>> {
    crate::scour::scour::<TaggedAcls>(value).map(|i| {
        i.into_iter()
            .map(|v| Scoured {
                path: v.path.join("acls"),
                value: v.value.into(),
            })
            .collect::<Vec<Scoured<Acls>>>()
    })
}

#[cfg(test)]
mod tests {
    use super::{find_acls, Acls, JsonValue, Scoured};
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
    pub fn it_should_return_basic_tagged_acl() {
        let acls = serde_json::to_value(json!({ "acls": Acls::default() })).unwrap();
        assert_eq!(
            find_acls(&acls),
            Some(vec![Scoured {
                path: vec!["acls"].into(),
                value: Acls::default()
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
            Some(vec![Scoured {
                path: vec!["nested", "acls"].into(),
                value: Acls::default()
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
                Scoured {
                    path: "scopes.hello.acls".into(),
                    value: Acls::default()
                },
                Scoured {
                    path: "scopes.world.acls".into(),
                    value: Acls::default()
                }
            ])
        );
    }
}
