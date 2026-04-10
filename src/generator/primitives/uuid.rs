pub struct UUIDGenerator {
    pub prefix: String,
    pub suffix: String,
}

impl UUIDGenerator {
    pub fn generate(&self) -> serde_json::Value {
        let generated_uuid = uuid::Uuid::new_v4();
        serde_json::json!(format!("{}{}{}", self.prefix, generated_uuid, self.suffix,))
    }
}
