use anyhow::Result;
use replies::JsonValue;
use std::{collections::HashMap, path::PathBuf, str::FromStr, time::Instant};
use tera::{Context, Tera};
use tracing::info;

#[derive(Clone, Debug)]
enum Node {
    File(PathBuf),
    Directory(HashMap<String, Node>),
}

impl Node {
    fn get(&self, key: &str) -> Option<&Node> {
        match self {
            Node::File(_) => None,
            Node::Directory(entries) => entries.get(key),
        }
    }

    fn path(&self) -> Option<&PathBuf> {
        match self {
            Node::File(path) => Some(path),
            Node::Directory(_) => None,
        }
    }

    fn paths(&self) -> impl Iterator<Item = PathBuf> {
        match self {
            Node::File(path) => {
                /*std::iter::once*/
                vec![path.clone()].into_iter()
            }
            Node::Directory(entries) => entries
                .values()
                .map(|c| c.paths())
                .flatten()
                .collect::<Vec<_>>()
                .into_iter(),
        }
    }

    fn find_match<'a>(&'a self, json: &'a JsonValue) -> Option<(&'a Node, String, &'a JsonValue)> {
        match json {
            JsonValue::Object(obj) => {
                if obj.len() == 1 {
                    let (key, value) = obj.into_iter().next().unwrap();
                    if let Some(child) = self.get(key) {
                        match child {
                            Node::File(_) => Some((child, key.clone(), value)),
                            Node::Directory(_) => {
                                child
                                    .find_match(value)
                                    .or(Some((child, key.clone(), value)))
                            }
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            _ => todo!("find-match\n{:#?}\n{:#?}", self, json),
        }
    }
}

#[derive(Clone)]
pub struct Renderer {
    tree: Node,
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
        */
        let mut tera = Tera::default();

        let directory = "text/replies";

        fn get_tree(dir: &std::path::Path) -> std::io::Result<Node> {
            let mut children = HashMap::new();
            if dir.is_dir() {
                for entry in std::fs::read_dir(dir)? {
                    let entry = entry?;
                    let path = entry.path();
                    let name = path.file_stem().unwrap().to_str().unwrap().to_string();

                    if path.is_dir() {
                        children.insert(name, get_tree(&path)?);
                    } else {
                        children.insert(name, Node::File(entry.path()));
                    }
                }
            }
            Ok(Node::Directory(children))
        }

        let tree = get_tree(&PathBuf::from_str(directory)?)?;

        info!("{:?}", &tree);

        {
            let files: Vec<_> = tree
                .paths()
                .map(|path| (path.clone(), None::<String>))
                .collect();
            tera.add_template_files(files)?;
        }

        tera.build_inheritance_chains()?;
        let elapsed = Instant::now() - started;
        info!(?elapsed, "compiled");

        Ok(Self { tree, tera })
    }

    pub fn render_value(&self, value: &JsonValue) -> Result<String> {
        let mut all = "".to_string();

        all.push('\n');

        let render = |context: Context, path: &str| -> Result<String> {
            let text = self.tera.render(path, &context)?;
            Ok(text.trim().to_owned())
        };

        match self.tree.find_match(value) {
            Some((node, key, value)) => {
                let name_from_path = node.path().unwrap().to_str().unwrap().to_owned();
                let mut context = Context::new();
                context.insert(key, &value);
                all.push_str(&render(context, &name_from_path)?);
            }
            None => tracing::warn!("No template:\n{:#?}!", value),
        }

        all.push('\n');

        Ok(all)
    }
}
