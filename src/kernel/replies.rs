use anyhow::Result;

pub type ReplyResult = Result<Box<dyn Reply>>;

pub use shared_replies::*;
