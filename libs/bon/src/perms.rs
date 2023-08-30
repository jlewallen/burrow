use serde::{Deserialize, Serialize};

use crate::{
    core::{DottedPath, JsonValue},
    scour::Scoured,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Subject {
    Everybody,
    Owner,
    Creator,
    Principal(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Perm {
    Read,
    Write,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AclRule {
    perm: Perm,
    sub: Vec<Subject>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Acls {
    #[serde(default)]
    //#[serde(skip)]
    rules: Vec<AclRule>,
}

impl Acls {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn new(rules: Vec<AclRule>) -> Self {
        Self { rules }
    }

    pub fn from_iter(rules: impl Iterator<Item = (Perm, Subject)>) -> Self {
        Self {
            rules: rules
                .map(|(perm, sub)| AclRule {
                    perm,
                    sub: vec![sub],
                })
                .collect(),
        }
    }
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

#[derive(Debug)]
pub struct SecurityContext<P> {
    pub actor: P,
    pub owner: P,
    pub creator: P,
}

impl<P> SecurityContext<P> {
    pub fn new(actor: P, owner: P, creator: P) -> Self {
        Self {
            actor,
            owner,
            creator,
        }
    }
}

impl<P> SecurityContext<P>
where
    P: PartialEq,
{
    pub(crate) fn allows(&self, subject: &Subject) -> bool {
        match subject {
            Subject::Everybody => true,
            Subject::Owner => self.actor == self.owner,
            Subject::Creator => self.actor == self.creator,
            Subject::Principal(_p) => todo!(),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Attempted {
    Write(DottedPath),
}

impl Attempted {
    fn perm(&self) -> Perm {
        match self {
            Attempted::Write(_) => Perm::Write,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Denied {
    Disallowed,
}

pub struct Policy<P> {
    acls: Vec<Scoured<Acls>>,
    context: SecurityContext<P>,
}

impl<P> Policy<P> {
    pub fn new(acls: Vec<Scoured<Acls>>, context: SecurityContext<P>) -> Self {
        Self { acls, context }
    }
}

impl<P> Policy<P>
where
    P: PartialEq,
{
    pub fn allows(&self, attempted: Attempted) -> Option<Denied> {
        if self.acls.is_empty() {
            return None;
        }

        match &attempted {
            Attempted::Write(path) => {
                let applying = self
                    .acls
                    .iter()
                    .filter(|v| v.path.drop_last().is_parent_of(path));

                let mut rulings: Vec<Option<Option<Denied>>> = Vec::new();

                for scoured in applying {
                    for rule in scoured
                        .value
                        .rules
                        .iter()
                        .filter(|rule| rule.perm == attempted.perm())
                    {
                        for sub in rule.sub.iter() {
                            if self.context.allows(sub) {
                                rulings.push(Some(None))
                            } else {
                                rulings.push(Some(Some(Denied::Disallowed)))
                            }
                        }
                    }
                }

                rulings.into_iter().flatten().flatten().next()
            }
        }
    }
}

pub trait HasSecurityContext<P> {
    fn security_context() -> SecurityContext<P>;
}

#[cfg(test)]
mod tests {
    mod application {
        use super::super::{
            Acls, Attempted, Denied, Perm, Policy, Scoured, SecurityContext, Subject,
        };

        #[test]
        pub fn it_should_allow_anything_when_empty() {
            let policy = Policy::new(vec![], SecurityContext::new("actor", "owner", "creator"));
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

            let allowing = Policy::new(
                acls.clone(),
                SecurityContext::new("actor", "owner", "nobody"),
            );
            let v = allowing.allows(Attempted::Write("scopes.props.name.value".into()));
            assert_eq!(v, None);
        }

        #[test]
        pub fn it_should_restrict_writes_to_owner() {
            let acls = vec![Scoured::new(
                "scopes.props.name.acls".into(),
                Acls::from_iter([(Perm::Write, Subject::Owner)].into_iter()),
            )];

            let failing = Policy::new(
                acls.clone(),
                SecurityContext::new("actor", "owner", "nobody"),
            );
            let v = failing.allows(Attempted::Write("scopes.props.name.value".into()));
            assert_eq!(v, Some(Denied::Disallowed));

            let allowing = Policy::new(
                acls.clone(),
                SecurityContext::new("owner", "owner", "nobody"),
            );
            let v = allowing.allows(Attempted::Write("scopes.props.name.value".into()));
            assert_eq!(v, None);

            let allowing = Policy::new(acls, SecurityContext::new("actor", "owner", "nobody"));
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
}
