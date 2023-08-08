use std::str::FromStr;

use crate::{building::model::QuickEdit, library::actions::*, looking::actions::LookAction};

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
                .into())
            }
            None => Ok(SimpleReply::NotFound.into()),
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
                let editing = editing.entity();
                Ok(EditorReply::new(
                    editing.key().to_string(),
                    WorkingCopy::Json(editing.to_json_value()?),
                    SaveEntityJsonAction::new_template(editing.key().clone())?,
                )
                .into())
            }
            None => Ok(SimpleReply::NotFound.into()),
        }
    }
}

#[action]
pub struct DescribeAction {
    pub item: Item,
}

impl Action for DescribeAction {
    fn is_read_only() -> bool {
        true
    }

    fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
        let (_, living, _) = surroundings.unpack();
        let action = PerformAction::Instance(Rc::new(EditAction {
            item: self.item.clone(),
        }));
        session.perform(Perform::Living { living, action })
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
                Ok(SimpleReply::Done.into())
            }
            None => Ok(SimpleReply::NotFound.into()),
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
                Ok(SimpleReply::Done.into())
            }
            None => Ok(SimpleReply::NotFound.into()),
        }
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
            .name(&self.new_area)
            .desc(&self.new_area)
            .try_into()?;
        let new_area = session.add_entity(&EntityPtr::new(new_area))?;

        let returning: Entity = build_entity()
            .exit()
            .name(&self.returning)
            .desc(&self.returning)
            .try_into()?;
        let returning = session.add_entity(&EntityPtr::new(returning))?;

        let outgoing: Entity = build_entity()
            .exit()
            .name(&self.outgoing)
            .desc(&self.outgoing)
            .try_into()?;
        let outgoing = session.add_entity(&EntityPtr::new(outgoing))?;

        tools::leads_to(&returning, &area)?;
        tools::set_container(&new_area, &vec![returning])?;

        tools::leads_to(&outgoing, &new_area)?;
        tools::set_container(&area, &vec![outgoing])?;

        // TODO Chain to GoAction?
        match tools::navigate_between(&area, &new_area, &living)? {
            DomainOutcome::Ok => session.perform(Perform::Living {
                living,
                action: PerformAction::Instance(Rc::new(LookAction {})),
            }),
            DomainOutcome::Nope => Ok(SimpleReply::NotFound.into()),
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

        let (_, user, _area) = surroundings.unpack();

        let new_item = EntityPtr::new_named(EntityClass::item(), &self.name, &self.name)?;

        session.add_entities(&[&new_item])?;

        tools::set_container(&user, &vec![new_item.try_into()?])?;

        Ok(SimpleReply::Done.into())
    }
}

#[derive(Debug, Serialize, Deserialize, ToJson)]
pub struct SaveQuickEditAction {
    pub key: EntityKey,
    pub copy: WorkingCopy,
}

impl SaveQuickEditAction {
    pub fn new(key: EntityKey, copy: WorkingCopy) -> Self {
        Self { key, copy }
    }

    pub fn new_template(key: EntityKey) -> Result<JsonTemplate, serde_json::Error> {
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

        match session.entry(&LookupBy::Key(&self.key))? {
            Some(entry) => {
                let entity = entry.entity();
                info!("save:quick-edit {:?}", entity);
                match &self.copy {
                    WorkingCopy::Markdown(text) => {
                        let quick = QuickEdit::from_str(text)?;
                        let mut entity = entity.borrow_mut();
                        if let Some(name) = quick.name {
                            entity.set_name(&name)?;
                        }
                        if let Some(desc) = quick.desc {
                            entity.set_desc(&desc)?;
                        }

                        Ok(SimpleReply::Done.into())
                    }
                    _ => Err(anyhow::anyhow!("Save expected JSON working copy")),
                }
            }
            None => Ok(SimpleReply::NotFound.into()),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, ToJson)]
pub struct SaveEntityJsonAction {
    pub key: EntityKey,
    pub copy: WorkingCopy,
}

impl SaveEntityJsonAction {
    pub fn new(key: EntityKey, copy: WorkingCopy) -> Self {
        Self { key, copy }
    }

    pub fn new_template(key: EntityKey) -> Result<JsonTemplate, serde_json::Error> {
        let copy = WorkingCopy::Json(serde_json::Value::String(
            JSON_TEMPLATE_VALUE_SENTINEL.to_owned(),
        ));
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

        match session.entry(&LookupBy::Key(&self.key))? {
            Some(entry) => {
                let entity = entry.entity();
                info!("save:entity-json {:?}", entity);
                match &self.copy {
                    WorkingCopy::Json(value) => {
                        let replacing = Entity::from_value(value.clone())?;
                        entity.replace(replacing);

                        Ok(SimpleReply::Done.into())
                    }
                    _ => Err(anyhow::anyhow!("Save expected JSON working copy")),
                }
            }
            None => Ok(SimpleReply::NotFound.into()),
        }
    }
}
