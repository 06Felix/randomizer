use crate::generation::StableRng;

#[derive(Debug)]
pub struct UUIDGenerator {
    pub(crate) prefix: String,
    pub(crate) suffix: String,
}

impl UUIDGenerator {
    pub(crate) fn generate(&self, rng: &mut StableRng) -> serde_json::Value {
        let mut bytes = [0_u8; 16];
        rng.fill_bytes(&mut bytes);
        bytes[6] = (bytes[6] & 0x0f) | 0x40;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        let generated_uuid = uuid::Uuid::from_bytes(bytes);
        serde_json::json!(format!("{}{}{}", self.prefix, generated_uuid, self.suffix,))
    }
}
