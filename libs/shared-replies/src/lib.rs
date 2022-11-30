use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct ObservedEntity {
    pub key: String,
    pub name: Option<String>,
    pub desc: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AreaObservation {
    pub area: ObservedEntity,
    pub person: ObservedEntity,
    pub living: Vec<ObservedEntity>,
    pub items: Vec<ObservedEntity>,
    pub carrying: Vec<ObservedEntity>,
    pub routes: Vec<ObservedEntity>,
}
