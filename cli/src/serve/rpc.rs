use kernel::Action;

pub fn try_parse_action(_value: serde_json::Value) -> anyhow::Result<Box<dyn Action>> {
    todo!()
}
