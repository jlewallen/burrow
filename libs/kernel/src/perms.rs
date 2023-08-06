use crate::Acls;

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::{
        perms::{find_acls, AclProtection},
        Acls,
    };

    #[test]
    pub fn it_should_return_none_json_primitives() {
        use serde_json::Value as V;
        assert!(find_acls(&V::Null).is_none());
        assert!(find_acls(&V::Bool(true)).is_none());
        assert!(find_acls(&V::Number(31337.into())).is_none());
        assert!(find_acls(&V::String("Hello".to_owned())).is_none());
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

#[allow(dead_code)]
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

pub fn find_acls(value: &serde_json::Value) -> Option<Vec<AclProtection>> {
    match value {
        serde_json::Value::Null => None,
        serde_json::Value::Bool(_) => None,
        serde_json::Value::Number(_) => None,
        serde_json::Value::String(_) => None,
        serde_json::Value::Array(array) => Some(
            array
                .iter()
                .map(|e| find_acls(e))
                .flatten()
                .flatten()
                .collect(),
        ),
        serde_json::Value::Object(o) => {
            let acls = serde_json::from_value::<Acls>(value.clone());
            match acls {
                Ok(acls) => Some(vec![AclProtection {
                    path: "".to_owned(),
                    acls,
                }]),
                Err(_) => Some(
                    o.iter()
                        .map(|(k, v)| find_acls(v).map(|o| o.into_iter().map(|p| p.prefix(k))))
                        .flatten()
                        .flatten()
                        .collect(),
                ),
            }
        }
    }
}

pub fn apply_read_acls(value: serde_json::Value) -> serde_json::Value {
    value
}
