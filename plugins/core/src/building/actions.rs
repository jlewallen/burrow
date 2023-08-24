use std::{rc::Rc, str::FromStr};

use chrono::Utc;

use crate::{
    building::model::{Constructed, QuickEdit},
    carrying::model::{Carryable, Containing},
    library::actions::*,
    looking::{actions::LookAction, model::new_area_observation},
    memory::model::{remember, EntityEvent, Memory},
    moving::model::Occupyable,
};

#[action]
pub struct AddScopeAction {
    pub scope_key: String,
}

impl Action for AddScopeAction {
    fn is_read_only() -> bool {
        false
    }

    fn perform(&self, _session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
        // TODO Security wise we would probably need a global list of who could
        // manipulate which scopes eventually.
        // TODO Right now this requires scopes to be functionable if all their
        // fields are ommitted. Look into `#[serde(default)]` to make this work
        // w/o a bunch of Option's?
        let Some(item) = tools::holding_one_item(surroundings.living())? else {
            return Ok(SimpleReply::NotFound.try_into()?);
        };

        debug!(item = ?item, scope_key = %self.scope_key, "add-scope");

        let mut item = item.entity().borrow_mut();

        item.add_scope_by_key(&self.scope_key);

        Ok(SimpleReply::Done.try_into()?)
    }
}

#[action]
pub struct EditAction {
    pub item: Item,
}

impl Action for EditAction {
    fn is_read_only() -> bool {
        true
    }

    fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
        debug!("editing {:?}!", self.item);

        match session.find_item(surroundings, &self.item)? {
            Some(editing) => {
                info!("editing {:?}", editing);
                let quick_edit: QuickEdit = (&editing).try_into()?;
                Ok(EditorReply::new(
                    editing.key().to_string(),
                    WorkingCopy::Markdown(quick_edit.to_string()),
                    SaveQuickEditAction::new_template(editing.key().clone())?,
                )
                .try_into()?)
            }
            None => Ok(SimpleReply::NotFound.try_into()?),
        }
    }
}

#[action]
pub struct EditRawAction {
    pub item: Item,
}

impl Action for EditRawAction {
    fn is_read_only() -> bool {
        true
    }

    fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
        debug!("editing {:?}!", self.item);

        match session.find_item(surroundings, &self.item)? {
            Some(editing) => {
                info!("editing {:?}", editing);
                let json = {
                    let editing = editing.borrow();
                    editing.to_json_value()?
                };
                let key = editing.key().clone();
                Ok(EditorReply::new(
                    key.to_string(),
                    WorkingCopy::Json(json),
                    SaveEntityJsonAction::new_template(key)?,
                )
                .try_into()?)
            }
            None => Ok(SimpleReply::NotFound.try_into()?),
        }
    }
}

#[action]
pub struct DuplicateAction {
    pub item: Item,
}

impl Action for DuplicateAction {
    fn is_read_only() -> bool {
        false
    }

    fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
        info!("duplicating {:?}!", self.item);

        match session.find_item(surroundings, &self.item)? {
            Some(duplicating) => {
                info!("duplicating {:?}", duplicating);
                _ = tools::duplicate(&duplicating)?;
                Ok(SimpleReply::Done.try_into()?)
            }
            None => Ok(SimpleReply::NotFound.try_into()?),
        }
    }
}

#[action]
pub struct ObliterateAction {
    pub item: Item,
}

impl Action for ObliterateAction {
    fn is_read_only() -> bool {
        false
    }

    fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
        info!("obliterate {:?}!", self.item);

        match session.find_item(surroundings, &self.item)? {
            Some(obliterating) => {
                info!("obliterate {:?}", obliterating);
                tools::obliterate(&obliterating)?;
                Ok(SimpleReply::Done.try_into()?)
            }
            None => Ok(SimpleReply::NotFound.try_into()?),
        }
    }
}

#[action]
pub struct MakeItemAction {
    pub name: String,
}

impl Action for MakeItemAction {
    fn is_read_only() -> bool {
        false
    }

    fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
        info!("make-item {:?}", self.name);

        let creator = surroundings.living();

        let new_item: Entity = build_entity()
            .default_scope::<Carryable>()?
            .creator(creator.entity_ref())
            .name(&self.name)
            .try_into()?;

        let new_item = session.add_entity(new_item)?;

        tools::set_quantity(&new_item, 1f32)?;
        tools::set_container(creator, &vec![new_item.clone()])?;

        remember(
            &creator,
            Utc::now(),
            Memory::Created(EntityEvent {
                key: new_item.key().clone(),
                gid: new_item.gid(),
                name: new_item.name()?.unwrap(),
            }),
        )?;

        Ok(SimpleReply::Done.try_into()?)
    }
}

#[action]
pub struct BidirectionalDigAction {
    pub outgoing: String,
    pub returning: String,
    pub new_area: String,
}

impl Action for BidirectionalDigAction {
    fn is_read_only() -> bool {
        false
    }

    fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
        info!(
            "bidirectional-dig {:?} <-> {:?} '{:?}'",
            self.outgoing, self.returning, self.new_area
        );

        let (_, living, area) = surroundings.unpack();

        let new_area: Entity = build_entity()
            .area()
            .default_scope::<Occupyable>()?
            .default_scope::<Containing>()?
            .name(&self.new_area)
            .desc(&self.new_area)
            .try_into()?;
        let new_area = session.add_entity(new_area)?;

        tools::add_route(&area, &self.outgoing, &new_area)?;
        tools::add_route(&new_area, &self.returning, &area)?;

        match tools::navigate_between(&area, &new_area, &living)? {
            true => Ok(session.perform(Perform::Living {
                living,
                action: PerformAction::Instance(Rc::new(LookAction {})),
            })?),
            false => Ok(SimpleReply::NotFound.try_into()?),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, ToTaggedJson, DeserializeTagged)]
pub struct SaveQuickEditAction {
    pub key: EntityKey,
    pub copy: WorkingCopy,
}

impl SaveQuickEditAction {
    pub fn new(key: EntityKey, copy: WorkingCopy) -> Self {
        Self { key, copy }
    }

    pub fn new_template(key: EntityKey) -> Result<JsonTemplate, TaggedJsonError> {
        let copy = WorkingCopy::Markdown(JSON_TEMPLATE_VALUE_SENTINEL.to_owned());
        let template = Self { key, copy };

        Ok(template.to_tagged_json()?.into())
    }
}

impl Action for SaveQuickEditAction {
    fn is_read_only() -> bool {
        false
    }

    fn perform(&self, session: SessionRef, _surroundings: &Surroundings) -> ReplyResult {
        info!("save:quick-edit {:?}", self.key);

        match session.entity(&LookupBy::Key(&self.key))? {
            Some(entity) => match &self.copy {
                WorkingCopy::Markdown(text) => {
                    let quick = QuickEdit::from_str(text)?;
                    let mut entity = entity.borrow_mut();
                    if let Some(name) = quick.name {
                        entity.set_name(&name)?;
                    }
                    if let Some(desc) = quick.desc {
                        entity.set_desc(&desc)?;
                    }

                    Ok(SimpleReply::Done.try_into()?)
                }
                _ => Err(anyhow::anyhow!("Save expected JSON working copy")),
            },
            None => Ok(SimpleReply::NotFound.try_into()?),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, ToTaggedJson, DeserializeTagged)]
pub struct SaveEntityJsonAction {
    pub key: EntityKey,
    pub copy: WorkingCopy,
}

impl SaveEntityJsonAction {
    pub fn new(key: EntityKey, copy: WorkingCopy) -> Self {
        Self { key, copy }
    }

    pub fn new_template(key: EntityKey) -> Result<JsonTemplate, TaggedJsonError> {
        let copy = WorkingCopy::Json(JsonValue::String(JSON_TEMPLATE_VALUE_SENTINEL.to_owned()));
        let template = Self { key, copy };

        Ok(template.to_tagged_json()?.into())
    }
}

impl Action for SaveEntityJsonAction {
    fn is_read_only() -> bool {
        false
    }

    fn perform(&self, session: SessionRef, _surroundings: &Surroundings) -> ReplyResult {
        info!("save:entity-json {:?}", self.key);

        match session.entity(&LookupBy::Key(&self.key))? {
            Some(entity) => match &self.copy {
                WorkingCopy::Json(value) => {
                    let replacing = Entity::from_value(value.clone())?;
                    entity.replace(replacing);

                    Ok(SimpleReply::Done.try_into()?)
                }
                _ => Err(anyhow::anyhow!("Save expected JSON working copy")),
            },
            None => Ok(SimpleReply::NotFound.try_into()?),
        }
    }
}

#[action]
pub struct BuildAreaAction {
    pub name: String,
}

impl Action for BuildAreaAction {
    fn is_read_only() -> bool {
        false
    }

    fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
        info!("build-area {:?}", self.name);

        let creator = surroundings.living();

        let new_area: Entity = build_entity()
            .area()
            .default_scope::<Containing>()?
            .default_scope::<Occupyable>()?
            .creator(creator.entity_ref())
            .name(&self.name)
            .try_into()?;

        let new_area = session.add_entity(new_area)?;

        remember(
            &creator,
            Utc::now(),
            Memory::Constructed(EntityEvent {
                key: new_area.key().clone(),
                gid: new_area.gid(),
                name: new_area.name()?.unwrap(),
            }),
        )?;

        info!("created {:?}", new_area);

        let observed = new_area_observation(creator, &new_area)?;
        let reply = Constructed::Area(observed);
        Ok(reply.try_into()?)
    }
}
