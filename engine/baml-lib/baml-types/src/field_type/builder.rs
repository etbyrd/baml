use super::{BamlMediaType, FieldType, TypeValue};

impl FieldType {
    pub fn string() -> Self {
        FieldType::Primitive(TypeValue::String)
    }

    pub fn literal_string(value: String) -> Self {
        FieldType::Literal(super::LiteralValue::String(value))
    }

    pub fn literal_int(value: i64) -> Self {
        FieldType::Literal(super::LiteralValue::Int(value))
    }

    pub fn literal_bool(value: bool) -> Self {
        FieldType::Literal(super::LiteralValue::Bool(value))
    }

    pub fn int() -> Self {
        FieldType::Primitive(TypeValue::Int)
    }

    pub fn float() -> Self {
        FieldType::Primitive(TypeValue::Float)
    }

    pub fn bool() -> Self {
        FieldType::Primitive(TypeValue::Bool)
    }

    pub fn null() -> Self {
        FieldType::Primitive(TypeValue::Null)
    }

    pub fn image() -> Self {
        FieldType::Primitive(TypeValue::Media(BamlMediaType::Image))
    }

    pub fn r#enum(name: &str) -> Self {
        FieldType::Enum(name.to_string())
    }

    pub fn class(name: &str) -> Self {
        FieldType::Class(name.to_string())
    }

    pub fn list(inner: FieldType) -> Self {
        FieldType::List(Box::new(inner))
    }

    pub fn as_list(self) -> Self {
        FieldType::List(Box::new(self))
    }

    pub fn map(key: FieldType, value: FieldType) -> Self {
        FieldType::Map(Box::new(key), Box::new(value))
    }

    pub fn union(choices: Vec<FieldType>) -> Self {
        FieldType::Union(choices)
    }

    pub fn tuple(choices: Vec<FieldType>) -> Self {
        FieldType::Tuple(choices)
    }

    pub fn optional(inner: FieldType) -> Self {
        FieldType::Optional(Box::new(inner))
    }

    pub fn as_optional(self) -> Self {
        FieldType::Optional(Box::new(self))
    }
}
