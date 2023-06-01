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
        {
            let directory = "text/replies";
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

        all.push('\n');

        let render = |context: Context, name: &str| -> Result<String> {
            let path = format!("text/replies/{}.txt", name);
            let text = self.tera.render(&path, &context)?;
            Ok(text.trim().to_owned())
        };

        match value {
            serde_json::Value::Object(object) => {
                for (key, value) in object {
                    let mut context = Context::new();
                    context.insert(key, &value);
                    all.push_str(&render(context, key)?);
                }
            }
            serde_json::Value::String(name) => {
                all.push_str(&render(Context::new(), name)?);
            }
            _ => todo!(),
        }

        all.push('\n');

        Ok(all)
    }

    pub fn render_reply(&self, reply: &Box<dyn Reply>) -> Result<String> {
        self.render_value(&reply.to_json()?)
    }
}
