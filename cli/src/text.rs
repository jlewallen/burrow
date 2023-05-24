use anyhow::Result;
use std::time::Instant;
use tera::{Context, Tera};
use tracing::info;

use replies::Reply;

pub struct Renderer {
    tera: Tera,
}

impl Renderer {
    pub fn new() -> Result<Self> {
        let started = Instant::now();
        // Doing the below is 2ms vs 350ms. It appears to be the globbing,
        // beause when I rereate the globbing here in a very simple fashion
        // things are slow again.
        // let tera = Tera::new("text/**/*")?;
        // let walker = globwalk::glob("text /**/*")?;
        /*
        for entry in walker.filter_map(std::result::Result::ok) {
            let path = entry.into_path();
            info!("{:?}", path.display())
        }

        tera.add_template_files(vec![
            ("text/replies/areaObservation.txt", None::<String>),
            ("text/replies/editor.txt", None::<String>),
            ("text/replies/impossible.txt", None::<String>),
            ("text/replies/notFound.txt", None::<String>),
            ("text/replies/what.txt", None::<String>),
            ("text/replies/done.txt", None::<String>),
            ("text/replies/simpleReply.txt", None::<String>),
        ])?;
        */
        let mut tera = Tera::default();
        for directory in ["text/replies"] {
            let files: Vec<_> = std::fs::read_dir(directory)?
                .filter_map(std::result::Result::ok)
                .map(|path| (path.path(), None::<String>))
                .collect();
            tera.add_template_files(files)?;
        }
        tera.build_inheritance_chains()?;
        let elapsed = Instant::now() - started;
        info!(?elapsed, "compiled");

        Ok(Self { tera })
    }

    pub fn render_value(&self, value: &serde_json::Value) -> Result<String> {
        let mut all = "".to_string();

        all.push_str("\n");

        match value {
            serde_json::Value::Object(object) => {
                for (key, value) in object {
                    let mut context = Context::new();
                    context.insert(key, &value);

                    let path = format!("text/replies/{}.txt", key);
                    let text = self.tera.render(&path, &context)?;
                    all.push_str(&text.trim());
                }
            }
            serde_json::Value::String(name) => {
                let context = Context::new();
                let path = format!("text/replies/{}.txt", name);
                let text = self.tera.render(&path, &context)?;
                all.push_str(&text.trim());
            }
            _ => todo!(),
        }

        all.push_str("\n");

        Ok(all)
    }

    pub fn render_reply(&self, reply: &Box<dyn Reply>) -> Result<String> {
        self.render_value(&reply.to_json()?)
    }
}
