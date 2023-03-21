use anyhow::Result;
use replies::Reply;
use tera::{Context, Tera};

pub struct Renderer {
    tera: Tera,
}

impl Renderer {
    pub fn new() -> Result<Self> {
        let tera = Tera::new("text/**/*")?;

        Ok(Self { tera })
    }

    pub fn render(&self, reply: Box<dyn Reply>) -> Result<String> {
        let mut all = "".to_string();

        match reply.to_json()? {
            serde_json::Value::Object(object) => {
                for (key, value) in object {
                    let mut context = Context::new();
                    context.insert(&key, &value);

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
}
