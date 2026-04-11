#![allow(dead_code)]
use super::model::Schema;

pub fn generate_schema_from_json_str(json: &str) -> Result<Schema, serde_json::Error> {
    serde_json::from_str(json)
}

#[cfg(test)]
mod tests {
    use super::generate_schema_from_json_str;
    use crate::schema::Schema;

    #[test]
    fn parses_nested_schema_from_json_string() {
        let json = r#"
        {
            "type": "object",
            "properties": {
                "age": {
                    "type": "int",
                    "min": 18,
                    "max": 99
                },
                "score": {
                    "type": "float",
                    "min": 0.5,
                    "max": 9.5,
                    "precision": 2
                }
            }
        }
        "#;

        let schema = generate_schema_from_json_str(json).expect("schema should parse");

        match schema {
            Schema::Object { properties } => {
                match properties.get("age") {
                    Some(Schema::Int { min, max }) => {
                        assert_eq!(*min, Some(18));
                        assert_eq!(*max, Some(99));
                    }
                    other => panic!("expected int schema for age, got {other:?}"),
                }

                match properties.get("score") {
                    Some(Schema::Float {
                        min,
                        max,
                        precision,
                    }) => {
                        assert_eq!(*min, Some(0.5));
                        assert_eq!(*max, Some(9.5));
                        assert_eq!(*precision, Some(2));
                    }
                    other => panic!("expected float schema for score, got {other:?}"),
                }
            }
            other => panic!("expected object schema, got {other:?}"),
        }
    }

    #[test]
    fn parses_list_schema_from_json_string() {
        let json = r#"
        {
            "type": "list",
            "min_length": 2,
            "max_length": 5,
            "items": {
                "type": "int",
                "min": 1,
                "max": 10
            }
        }
        "#;

        let schema = generate_schema_from_json_str(json).expect("schema should parse");

        match schema {
            Schema::List {
                length,
                min_length,
                max_length,
                items,
            } => {
                assert_eq!(length, None);
                assert_eq!(min_length, Some(2));
                assert_eq!(max_length, Some(5));

                match items.as_ref() {
                    Schema::Int { min, max } => {
                        assert_eq!(*min, Some(1));
                        assert_eq!(*max, Some(10));
                    }
                    other => panic!("expected int schema for items, got {other:?}"),
                }
            }
            other => panic!("expected list schema, got {other:?}"),
        }
    }

    #[test]
    fn returns_error_for_invalid_json_string() {
        let json = r#"{ "type": "int", "min": 1, }"#;

        let result = generate_schema_from_json_str(json);

        assert!(result.is_err());
    }
}
