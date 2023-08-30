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

    pub fn drop_last(&self) -> Self {
        Self(
            self.0[0..self.0.len().saturating_sub(1)]
                .iter()
                .map(|v| v.to_owned())
                .collect(),
        )
    }

    pub fn is_parent_of(&self, other: &Self) -> bool {
        if self.0.len() <= other.0.len() {
            other.0[0..self.0.len()] == self.0
        } else {
            false
        }
    }
}

impl From<Vec<String>> for DottedPath {
    fn from(value: Vec<String>) -> Self {
        Self(value.into_iter().collect())
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
