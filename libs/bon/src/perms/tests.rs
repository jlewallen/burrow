mod application {
    use super::super::{Acls, Attempted, Denied, Perm, Policy, Scoured, SecurityContext, Subject};

    #[test]
    pub fn it_should_allow_anything_when_empty() {
        let acls = Vec::new();
        let policy = Policy::new(&acls, SecurityContext::new("actor", "owner", "creator"));
        assert_eq!(
            policy.allows(Attempted::Write("scopes.containing".into())),
            None
        );
        assert_eq!(policy.allows(Attempted::Write("scopes".into())), None);
        assert_eq!(policy.allows(Attempted::Write("owner".into())), None);
    }

    #[test]
    pub fn it_should_always_allow_everybody() {
        let acls = vec![Scoured::new(
            "scopes.props.name.acls".into(),
            Acls::from_iter([(Perm::Write, Subject::Everybody)].into_iter()),
        )];

        let allowing = Policy::new(&acls, SecurityContext::new("actor", "owner", "nobody"));
        let v = allowing.allows(Attempted::Write("scopes.props.name.value".into()));
        assert_eq!(v, None);
    }

    #[test]
    pub fn it_should_restrict_writes_to_owner() {
        let acls = vec![Scoured::new(
            "scopes.props.name.acls".into(),
            Acls::from_iter([(Perm::Write, Subject::Owner)].into_iter()),
        )];

        let failing = Policy::new(&acls, SecurityContext::new("actor", "owner", "nobody"));
        let v = failing.allows(Attempted::Write("scopes.props.name.value".into()));
        assert_eq!(v, Some(Denied::Disallowed));

        let allowing = Policy::new(&acls, SecurityContext::new("owner", "owner", "nobody"));
        let v = allowing.allows(Attempted::Write("scopes.props.name.value".into()));
        assert_eq!(v, None);

        let allowing = Policy::new(&acls, SecurityContext::new("actor", "owner", "nobody"));
        let v = allowing.allows(Attempted::Write("scopes.props.desc.value".into()));
        assert_eq!(v, None);
    }
}

mod parsing {
    use super::super::{find_acls, Acls, JsonValue, Scoured};
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
