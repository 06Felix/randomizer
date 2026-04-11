use rand::{Rng, RngExt};

pub enum StringGeneratorMode {
    Charset {
        min_length: usize,
        max_length: usize,
        charset: Vec<char>,
    },
    Enum {
        values: Vec<String>,
    },
}

pub struct StringGenerator {
    pub prefix: String,
    pub suffix: String,
    pub mode: StringGeneratorMode,
}

impl StringGenerator {
    pub fn generate(&self, rng: &mut impl Rng) -> serde_json::Value {
        let value = match &self.mode {
            StringGeneratorMode::Charset {
                min_length,
                max_length,
                charset,
            } => {
                let length = rng.random_range(*min_length..=*max_length);
                let mut value = String::with_capacity(length);

                for _ in 0..length {
                    let index = rng.random_range(0..charset.len());
                    value.push(charset[index]);
                }

                value
            }
            StringGeneratorMode::Enum { values } => {
                let index = rng.random_range(0..values.len());
                values[index].clone()
            }
        };

        serde_json::json!(format!("{}{}{}", self.prefix, value, self.suffix))
    }
}
