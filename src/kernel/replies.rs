use anyhow::Result;
use markdown_gen::markdown;
use serde::Serialize;
use std::string::FromUtf8Error;

pub type ReplyResult = Result<Box<dyn Reply>>;

pub type Markdown = markdown::Markdown<Vec<u8>>;

pub fn markdown_to_string(md: Markdown) -> Result<String, FromUtf8Error> {
    String::from_utf8(md.into_inner())
}

pub trait Reply: std::fmt::Debug + erased_serde::Serialize {
    fn to_markdown(&self) -> Result<Markdown>;
}

#[derive(Debug, Serialize)]
pub enum SimpleReply {
    Done,
}

impl Reply for SimpleReply {
    fn to_markdown(&self) -> Result<Markdown> {
        let mut md = Markdown::new(Vec::new());
        md.write("ok!")?;
        Ok(md)
    }
}
