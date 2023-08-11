use std::{fs::File, io::Read, path::Path, rc::Rc};

use anyhow::{anyhow, Result};
use tracing::info;

use engine::prelude::Session;
use kernel::prelude::JsonValue;

pub struct Renderer {
    target: crate::text::Renderer,
    #[allow(dead_code)]
    session: Rc<Session>,
}

impl Renderer {
    pub fn new(session: Rc<Session>, target: crate::text::Renderer) -> Result<Self> {
        Ok(Self { target, session })
    }

    pub fn render_value(&self, value: &JsonValue) -> Result<String> {
        self.target.render_value(value)
    }
}

pub fn default_external_editor(value: &str, extension: &str) -> Result<String> {
    external_editor::<HelixEditor>(value, extension)
}

fn external_editor<T>(value: &str, extension: &str) -> Result<String>
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

trait ExternalEditor {
    fn run(&self, path: &Path) -> Result<()>;
}

#[allow(dead_code)]
#[derive(Default)]
struct HelixEditor {}

impl ExternalEditor for HelixEditor {
    fn run(&self, path: &Path) -> Result<()> {
        spawn_editor("hx", path)
    }
}

#[allow(dead_code)]
#[derive(Default)]
struct VimEditor {}

impl ExternalEditor for VimEditor {
    fn run(&self, path: &Path) -> Result<()> {
        spawn_editor("vim", path)
    }
}

#[allow(dead_code)]
#[derive(Default)]
struct VsCodeEditor {}

impl ExternalEditor for VsCodeEditor {
    fn run(&self, path: &Path) -> Result<()> {
        spawn_editor("code -w", path)
    }
}

fn spawn_editor(prefix: &str, path: &Path) -> Result<()> {
    info!("editor:spawn '{} {}'", prefix, path.display());

    let status = std::process::Command::new("/bin/sh")
        .arg("-c")
        // Note that this is passed as one argument to the
        // shell's -c argument and that is why multiple arg
        // calls aren't being used.
        .arg(format!("{} {}", prefix, path.display()))
        .spawn()
        .map_err(|_| anyhow!("Error: Failed to run /bin/sh -c {}", prefix))?
        .wait()
        .expect("Error: Editor returned a non-zero status");

    info!("editor:done {:?}", status);

    Ok(())
}
