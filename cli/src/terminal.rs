use std::{fs::File, io::Read, path::Path, rc::Rc};

use anyhow::{anyhow, Result};
use tracing::info;

use engine::Session;
use replies::Reply;

pub struct Renderer {
    text: crate::text::Renderer,
    #[allow(dead_code)]
    session: Rc<Session>,
}

impl Renderer {
    pub fn new(session: Rc<Session>) -> Result<Self> {
        Ok(Self {
            text: crate::text::Renderer::new()?,
            session,
        })
    }

    pub fn render_reply(&self, reply: &Box<dyn Reply>) -> Result<String> {
        let value = reply.to_json()?;
        match &value {
            serde_json::Value::Object(object) => {
                for (key, value) in object {
                    // TODO This is annoying.
                    if key == "editor" {
                        info!("{:?}", value);

                        let edited =
                            self.external_editor::<TerminalVimEditor>("hello world", "txt")?;

                        info!("{:?}", edited);

                        return Ok("".to_string());
                    }
                }
                self.text.render_value(&value)
            }
            _ => self.text.render_value(&value),
        }
    }

    fn external_editor<T>(&self, value: &str, extension: &str) -> Result<String>
    where
        T: ExternalEditor + Default,
    {
        use std::io::Write;

        let dir = tempfile::tempdir()?;
        let file_path = dir.path().join(format!("editing.{}", extension));
        let mut file = File::create(&file_path)?;
        write!(file, "{}", value)?;
        file.flush()?;

        let editor: T = T::default();
        editor.run(&file_path)?;

        let mut edited = String::new();
        let mut file = File::open(file_path)?;
        file.read_to_string(&mut edited)?;

        Ok(edited)
    }
}

trait ExternalEditor {
    fn run(&self, path: &Path) -> Result<()>;
}

#[allow(dead_code)]
#[derive(Default)]
struct TerminalVimEditor {}

impl ExternalEditor for TerminalVimEditor {
    fn run(&self, path: &Path) -> Result<()> {
        info!("opening in vim and waiting on close");

        let status = std::process::Command::new("/bin/sh")
            .arg("-c")
            // Note that this is passed as one argument to the
            // shell's -c argument and that is why multiple arg
            // calls aren't being used.
            .arg(format!("vim {}", path.display()))
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
    fn run(&self, path: &Path) -> Result<()> {
        info!("opening in vscode and waiting on close");

        let status = std::process::Command::new("/bin/sh")
            .arg("-c")
            // Note that this is passed as one argument to the
            // shell's -c argument and that is why multiple arg
            // calls aren't being used.
            .arg(format!("code -w {}", path.display()))
            .spawn()
            .or_else(|_| Err(anyhow!("Error: Failed to run /bin/sh -c code")))?
            .wait()
            .expect("Error: Editor returned a non-zero status");

        info!("finished: {:?}", status);

        Ok(())
    }
}
