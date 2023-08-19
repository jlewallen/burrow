use kernel::prelude::{Audience, EntityKey, Raised};
use serde_json::json;

use crate::sources::{Owner, Relation};

use super::*;

#[test]
pub fn test_handlers_apply() -> Result<()> {
    let source = r#"
            pub fn held(bag) { }

            pub fn dropped(bag) { }

            pub fn left(bag) { }

            pub fn arrived(bag) { }

            pub fn handlers() {
                #{
                    "carrying": #{
                        "held": held,
                        "dropped": dropped
                    },
                    "moving": #{
                        "left": left,
                        "arrived": arrived
                    }
                }
            }
        "#;

    let mut runner = RuneRunner::new(Script {
        source: ScriptSource::System(source.to_owned()),
        owner: None,
    })?;

    runner.before(Perform::Raised(Raised::new(
        Audience::Nobody, // Unused
        "UNUSED".to_owned(),
        TaggedJson::new_from(json!({
            "carrying": {
                "dropped": {
                    "item": {
                        "name": "Dropped Item",
                        "key": "E-0"
                    }
                }
            }
        }))?,
    )))?;

    Ok(())
}

#[test]
pub fn test_missing_handler() -> Result<()> {
    let source = r#"
            pub fn handlers() {
                #{ }
            }
        "#;

    let mut runner = RuneRunner::new(Script {
        source: ScriptSource::System(source.to_owned()),
        owner: None,
    })?;

    runner.before(Perform::Raised(Raised::new(
        Audience::Nobody, // Unused
        "UNUSED".to_owned(),
        TaggedJson::new_from(json!({
            "carrying": {
                "dropped": {
                    "item": {
                        "name": "Dropped Item",
                        "key": "E-0"
                    }
                }
            }
        }))?,
    )))?;

    Ok(())
}

#[test]
pub fn test_missing_handlers_completely() -> Result<()> {
    let source = r#" "#;

    let mut runner = RuneRunner::new(Script {
        source: ScriptSource::System(source.to_owned()),
        owner: None,
    })?;

    runner.before(Perform::Raised(Raised::new(
        Audience::Nobody, // Unused
        "UNUSED".to_owned(),
        TaggedJson::new_from(json!({
            "carrying": {
                "dropped": {
                    "item": {
                        "name": "Dropped Item",
                        "key": "E-0"
                    }
                }
            }
        }))?,
    )))?;

    Ok(())
}

#[test]
pub fn test_calling_owner_with_one() -> Result<()> {
    let source = r#"
            pub fn held(bag) {
                info(format!("{:?}", owner()))
            }

            pub fn handlers() {
                #{
                    "carrying": #{
                        "held": held,
                    },
                }
            }
        "#;

    let mut runner = RuneRunner::new(Script {
        source: ScriptSource::System(source.to_owned()),
        owner: Some(Owner::new(EntityKey::new("E-0"), Relation::Ground)),
    })?;

    runner.before(Perform::Raised(Raised::new(
        Audience::Nobody, // Unused
        "UNUSED".to_owned(),
        TaggedJson::new_from(json!({
            "carrying": {
                "held": {
                    "item": {
                        "name": "Dropped Item",
                        "key": "E-0"
                    }
                }
            }
        }))?,
    )))?;

    Ok(())
}

#[test]
pub fn test_calling_owner_with_none() -> Result<()> {
    let source = r#"
            pub fn held(bag) {
                info(format!("{:?}", owner()))
            }

            pub fn handlers() {
                #{
                    "carrying": #{
                        "held": held,
                    },
                }
            }
        "#;

    let mut runner = RuneRunner::new(Script {
        source: ScriptSource::System(source.to_owned()),
        owner: None,
    })?;

    runner.before(Perform::Raised(Raised::new(
        Audience::Nobody, // Unused
        "UNUSED".to_owned(),
        TaggedJson::new_from(json!({
            "carrying": {
                "held": {
                    "item": {
                        "name": "Dropped Item",
                        "key": "E-0"
                    }
                }
            }
        }))?,
    )))?;

    Ok(())
}

#[test]
pub fn test_chain() -> Result<()> {
    let living = build_entity()
        .living()
        .with_key(EntityKey::new("E-0"))
        .identity(Identity::new("".to_lowercase(), "".to_owned()))
        .try_into()?;
    let perform = Perform::Living {
        living: EntityPtr::new_from_entity(living),
        action: PerformAction::TaggedJson(TaggedJson::new_from(json!({
            "lookAction": { }
        }))?),
    };

    println!("{:#?}", perform);

    Ok(())
}
