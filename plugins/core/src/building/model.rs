use crate::library::model::*;

use std::str::FromStr;

#[derive(Debug, Serialize, ToTaggedJson)]
#[serde(rename_all = "camelCase")]
struct EditorReply {}

impl Reply for EditorReply {}

#[derive(Default, Clone, Debug)]
pub struct QuickEdit {
    pub name: Option<String>,
    pub desc: Option<String>,
}

impl TryFrom<&Entry> for QuickEdit {
    type Error = DomainError;

    fn try_from(value: &Entry) -> std::result::Result<Self, Self::Error> {
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
            self.name.as_ref().map(|s| s.as_str()).unwrap_or(""),
            SEPARATOR,
            self.desc.as_ref().map(|s| s.as_str()).unwrap_or("")
        )
    }
}
