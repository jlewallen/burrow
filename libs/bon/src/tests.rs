use serde::Serialize;
use serde_json::json;

use crate::prelude::*;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum HelloWorld {
    Message(String),
}

impl HasTag for HelloWorld {
    fn tag() -> std::borrow::Cow<'static, str>
    where
        Self: Sized,
    {
        "helloWorld".into()
    }
}

impl ToTaggedJson for HelloWorld {
    fn to_tagged_json(&self) -> Result<TaggedJson, TaggedJsonError> {
        let value = serde_json::to_value(self)?;
        Ok(TaggedJson::new(Self::tag().to_string(), value.into()))
    }
}

#[test]
pub fn test_to_json_tags_enum() {
    assert_eq!(
        HelloWorld::Message("Hey!".to_owned())
            .to_tagged_json()
            .expect("ToTaggedJson failed"),
        TaggedJson::new("helloWorld".to_owned(), json!({ "message": "Hey!"}).into())
    );
}

#[test]
pub fn test_serialize_tagged_json() {
    let tagged = TaggedJson::new(
        "thing".to_owned(),
        Json(JsonValue::String("value".to_owned())),
    );
    assert_eq!(
        serde_json::to_value(&tagged).unwrap(),
        json!({ "thing": "value" })
    );
    assert_eq!(
        serde_json::from_value::<TaggedJson>(json!({ "thing": "value" })).unwrap(),
        tagged
    );
}
