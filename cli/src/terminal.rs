use anyhow::{anyhow, Result};
use tracing::info;

use replies::Reply;

pub struct Renderer {
    text: crate::text::Renderer,
}

impl Renderer {
    pub fn new() -> Result<Self> {
        Ok(Self {
            text: crate::text::Renderer::new()?,
        })
    }

    pub fn render_reply(&self, reply: &Box<dyn Reply>) -> Result<String> {
        let value = reply.to_json()?;
        match &value {
            serde_json::Value::Object(object) => {
                for (key, value) in object {
                    if key == "editor" {
                        info!("{:?}", value);

                        self.external_editor::<VsCodeEditor>()?;

                        return Ok("".to_string());
                    }
                }
                self.text.render_value(&value)
            }
            _ => self.text.render_value(&value),
        }
    }

    fn external_editor<T>(&self) -> Result<()>
    where
        T: ExternalEditor + Default,
    {
        let editor: T = T::default();

        editor.run("justfile")?;

        Ok(())
    }
}

trait ExternalEditor {
    fn run(&self, path: &str) -> Result<()>;
}

#[allow(dead_code)]
#[derive(Default)]
struct TerminalVimEditor {}

impl ExternalEditor for TerminalVimEditor {
    fn run(&self, path: &str) -> Result<()> {
        info!("opening in vim and waiting on close");

        let status = std::process::Command::new("/bin/sh")
            .arg("-c")
            // Note that this is passed as one argument to the
            // shell's -c argument and that is why multiple arg
            // calls aren't being used.
            .arg(format!("vim {}", path))
            .spawn()
            .or_else(|_| Err(anyhow!("Error: Failed to run /bin/sh -c vim")))?
            .wait()
            .expect("Error: Editor returned a non-zero status");

        info!("finished: {:?}", status);

        Ok(())
    }
}

#[allow(dead_code)]
#[derive(Default)]
struct VsCodeEditor {}

impl ExternalEditor for VsCodeEditor {
    fn run(&self, path: &str) -> Result<()> {
        info!("opening in vscode and waiting on close");

        let status = std::process::Command::new("/bin/sh")
            .arg("-c")
            // Note that this is passed as one argument to the
            // shell's -c argument and that is why multiple arg
            // calls aren't being used.
            .arg(format!("code -w {}", path))
            .spawn()
            .or_else(|_| Err(anyhow!("Error: Failed to run /bin/sh -c code")))?
            .wait()
            .expect("Error: Editor returned a non-zero status");

        info!("finished: {:?}", status);

        Ok(())
    }
}
