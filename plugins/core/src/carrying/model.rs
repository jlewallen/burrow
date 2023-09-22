use crate::{library::model::*, tools};

pub use kernel::common::Carrying;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Containing {
    pub(crate) holding: Vec<EntityRef>,
    pub(crate) capacity: Option<u32>,
    pub(crate) produces: HashMap<String, String>,
}

impl Scope for Containing {
    fn scope_key() -> &'static str {
        "containing"
    }
}

impl Containing {
    pub fn start_carrying(&mut self, item: &EntityPtr) -> Result<bool, DomainError> {
        if let Some(carryable) = item.scope::<Carryable>()? {
            let holding = self
                .holding
                .iter()
                .map(|h| h.to_entity())
                .collect::<Result<Vec<_>, _>>()?;

            for held in holding {
                if is_kind(&held, &carryable.kind)? {
                    let mut combining = held.scope_mut::<Carryable>()?;

                    combining.increase_quantity(&carryable.quantity.into())?;

                    combining.save()?;

                    get_my_session()?.obliterate(item)?;

                    return Ok(true);
                }
            }
        }

        if !self.is_holding(item) {
            self.holding.push(item.entity_ref());
        }

        Ok(true)
    }

    pub fn is_holding(&self, item: &EntityPtr) -> bool {
        self.holding.iter().any(|i| *i.key() == item.key())
    }

    fn remove_item(&mut self, item: &EntityPtr) -> Result<bool, DomainError> {
        self.holding = self
            .holding
            .iter()
            .flat_map(|i| {
                if *i.key() == item.key() {
                    vec![]
                } else {
                    vec![i.clone()]
                }
            })
            .collect::<Vec<EntityRef>>()
            .to_vec();

        Ok(true)
    }

    pub fn stop_carrying(&mut self, found: Found) -> Result<Option<EntityPtr>, DomainError> {
        let item = found.entity()?;

        if !self.is_holding(item) {
            return Ok(None);
        }

        if let Some(carryable) = item.scope::<Carryable>()? {
            match &found {
                Found::One(item) => {
                    if carryable.quantity > 1.0 {
                        let (_original, separated) = tools::separate(item, &1.0.into())?;

                        Ok(Some(separated))
                    } else {
                        self.remove_item(item)?;

                        Ok(Some(item.clone()))
                    }
                }
                Found::Quantified(q, entity) => {
                    if carryable.quantity > q.as_f32() {
                        let (_original, separated) = tools::separate(entity, q)?;

                        Ok(Some(separated))
                    } else {
                        Ok(None)
                    }
                }
            }
        } else {
            self.remove_item(item)?;

            Ok(Some(item.clone()))
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Carryable {
    kind: Kind,
    quantity: f32,
}

fn is_kind(entity: &EntityPtr, kind: &Kind) -> Result<bool, DomainError> {
    if let Some(carryable) = entity.scope::<Carryable>()? {
        Ok(*carryable.kind() == *kind)
    } else {
        Ok(false)
    }
}

impl Default for Carryable {
    fn default() -> Self {
        let session = get_my_session().expect("No session in Entity::new_blank!");
        Self {
            kind: Kind::new(session.new_identity()),
            quantity: 1.0,
        }
    }
}

impl Carryable {
    pub fn quantity(&self) -> f32 {
        self.quantity
    }

    pub fn decrease_quantity(&mut self, q: &Quantity) -> Result<&mut Self, DomainError> {
        self.sanity_check_quantity();

        if *q < 1.0.into() || *q > self.quantity.into() {
            Err(DomainError::Impossible)
        } else {
            self.quantity -= q.as_f32();

            Ok(self)
        }
    }

    pub fn increase_quantity(&mut self, q: &Quantity) -> Result<&mut Self, DomainError> {
        self.sanity_check_quantity();

        self.quantity += q.as_f32();

        Ok(self)
    }

    pub fn set_quantity(&mut self, q: &Quantity) -> Result<&mut Self, DomainError> {
        self.quantity = q.as_f32();

        Ok(self)
    }

    pub fn kind(&self) -> &Kind {
        &self.kind
    }

    pub fn set_kind(&mut self, kind: &Kind) {
        self.kind = kind.clone();
    }

    // Migrate items that were initialized with 0 quantities.
    fn sanity_check_quantity(&mut self) {
        if self.quantity < 1.0 {
            self.quantity = 1.0
        }
    }
}

impl Scope for Carryable {
    fn scope_key() -> &'static str {
        "carryable"
    }
}
