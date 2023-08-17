use crate::library::model::*;

use std::str::FromStr;

#[derive(Clone, Serialize, Deserialize, PartialEq, ToTaggedJson, Reply, Debug)]
#[serde(rename_all = "camelCase")]
pub enum Constructed {
    Area(AreaObservation),
}

impl TryFrom<Constructed> for Effect {
    type Error = TaggedJsonError;

    fn try_from(value: Constructed) -> std::result::Result<Self, Self::Error> {
        Ok(Self::Reply(value.to_tagged_json()?.into()))
    }
}

#[derive(Debug, Serialize, ToTaggedJson)]
#[serde(rename_all = "camelCase")]
struct EditorReply {}

impl Reply for EditorReply {}

#[derive(Default, Clone, Debug)]
pub struct QuickEdit {
    pub name: Option<String>,
    pub desc: Option<String>,
}

impl TryFrom<&EntityPtr> for QuickEdit {
    type Error = DomainError;

    fn try_from(value: &EntityPtr) -> std::result::Result<Self, Self::Error> {
        let name = value.name()?;
        let desc = value.desc()?;
        Ok(Self { name, desc })
    }
}

const SEPARATOR: &str =
    "~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~";

impl FromStr for QuickEdit {
    type Err = DomainError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let parts: Vec<_> = s.split(SEPARATOR).collect();
        if parts.len() != 2 {
            return Err(DomainError::Anyhow(anyhow::anyhow!("malformed quick edit")));
        }

        let (name, desc) = match parts[..] {
            [name, desc] => (name, desc),
            _ => todo!(),
        };

        let name = Some(name.trim().to_owned());
        let desc = Some(desc.trim().to_owned());

        Ok(Self { name, desc })
    }
}

impl ToString for QuickEdit {
    fn to_string(&self) -> String {
        format!(
            "{}\n\n{}\n\n{}",
            self.name.as_deref().unwrap_or(""),
            SEPARATOR,
            self.desc.as_deref().unwrap_or("")
        )
    }
}
