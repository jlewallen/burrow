use serde::{Deserialize, Serialize};

use crate::{core::JsonValue, scour::Scoured};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AclRule {
    keys: Vec<String>,
    perm: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Acls {
    rules: Vec<AclRule>,
}

pub fn find_acls(value: &JsonValue) -> Option<Vec<Scoured<Acls>>> {
    crate::scour::scour(value)
}

pub fn apply_read_acls(value: JsonValue) -> JsonValue {
    value
}

#[cfg(test)]
mod tests {
    use crate::core::DottedPath;

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
    pub fn it_should_return_basic_acl() {
        let acls = serde_json::to_value(Acls::default()).unwrap();
        assert_eq!(
            find_acls(&acls),
            Some(vec![Scoured {
                path: DottedPath::default(),
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
