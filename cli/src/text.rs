use anyhow::Result;
use tera::{Context, Tera};

use replies::Reply;

pub struct Renderer {
    tera: Tera,
}

impl Renderer {
    pub fn new() -> Result<Self> {
        let tera = Tera::new("text/**/*")?;

        Ok(Self { tera })
    }

    pub fn render_value(&self, value: &serde_json::Value) -> Result<String> {
        let mut all = "".to_string();

        match value {
            serde_json::Value::Object(object) => {
                for (key, value) in object {
                    let mut context = Context::new();
                    context.insert(key, &value);

                    let path = format!("replies/{}.txt", key);
                    let text = self.tera.render(&path, &context)?;
                    all.push_str(&text);
                }
            }
            serde_json::Value::String(name) => {
                let context = Context::new();
                let path = format!("replies/{}.txt", name);
                let text = self.tera.render(&path, &context)?;
                all.push_str(&text);
            }
            _ => todo!(),
        }

        Ok(all)
    }

    pub fn render_reply(&self, reply: &Box<dyn Reply>) -> Result<String> {
        self.render_value(&reply.to_json()?)
    }
}
