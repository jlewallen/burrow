use serde::{Deserialize, Serialize};

use crate::{
    dotted::{DottedPath, JsonValue},
    scour::Scoured,
};

#[cfg(test)]
mod tests;

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

#[derive(Clone, Debug)]
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

pub struct Policy<'a, P> {
    acls: &'a Vec<Scoured<Acls>>,
    context: SecurityContext<P>,
}

impl<'a, P> Policy<'a, P> {
    pub fn new(acls: &'a Vec<Scoured<Acls>>, context: SecurityContext<P>) -> Self {
        Self { acls, context }
    }
}

impl<'a, P> Policy<'a, P>
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
    fn security_context(&self) -> SecurityContext<P>;
}
