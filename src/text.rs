use crate::kernel::Reply;
use anyhow::Result;
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
        let maybe_tree = reply.to_json()?;
        if let Some(tree) = maybe_tree.as_object() {
            for (key, value) in tree {
                let mut context = Context::new();
                context.insert(key, value);

                let path = format!("replies/{}.txt", key);
                let text = self.tera.render(&path, &context)?;

                all.push_str(&text);
            }
        }

        Ok(all)
    }
}
