//! value representation
//!
//! The cco output model contains the following data types
//! - boolean (true/false)
//! - integer (signed, currently: i64 - may change)
//! - decimal (currently: f64 - may change)
//! - string (utf-8)
//! - array ("list" of values)
//! - object (order-preserving "map"/"dictionary", where the key is of type string)
//!
//! Additionally:
//! - there is no `null`/`None` value.
//! - the only valid **implicit** conversion: every `integer` is also a `decimal`
//! - numeric type ranges (min/max) for `integer` or `decimal` are currently not defined and are subject to change
//!
//! TODO: Currently we pretend that `null` or out-of-bounds integers do not exist.
//!
use serde::{
    ser::{SerializeMap, SerializeSeq},
    Serializer,
};

/// All possible value types
#[derive(Debug, Clone)]
pub enum Value {
    Boolean(bool),
    Integer(i64),
    Decimal(f64),
    String(String),
    Array(Vec<Value>),
    Object(indexmap::IndexMap<String, Value>),
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Value::String(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Value::String(value.to_string())
    }
}

impl From<hcl::Body> for Value {
    fn from(value: hcl::Body) -> Self {
        Value::Object(
            value
                .into_attributes()
                .map(|next| (next.key.to_string(), next.expr.into()))
                .collect(),
        )
    }
}

impl From<hcl::Expression> for Value {
    fn from(value: hcl::Expression) -> Self {
        use hcl::Expression;

        match value {
            Expression::Bool(bool) => bool.into(),
            Expression::Number(num) => {
                if num.is_f64() {
                    return Value::Decimal(num.as_f64().expect(
                        "is_f64 said that number is a float but as_f64 did not return it as such",
                    ));
                }
                if let Some(int) = num.as_i64() {
                    return Value::Integer(int);
                }

                // FIXME: We pretend that large numbers are never used
                panic!("out of bounds integer");
            }
            Expression::String(s) => s.into(),
            Expression::Array(array) => array.into(),
            Expression::Object(object) => object.into(),
            Expression::Null => {
                // TODO: Don't panic. Handle errors.
                panic!("null value found. This should never happen. Please report this.")
            }
            _ => {
                // TODO: Don't panic. Handle errors.
                panic!("unresolved hcl expression found. This should never happen. Please report this.")
            }
        }
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Self::Boolean(value)
    }
}

impl<K: ToString, V: Into<Value>> From<hcl::value::Map<K, V>> for Value {
    fn from(value: hcl::value::Map<K, V>) -> Self {
        Value::Object(
            value
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.into()))
                .collect(),
        )
    }
}

impl From<hcl::Number> for Value {
    fn from(value: hcl::Number) -> Self {
        if let Some(int) = value.as_i64() {
            return Value::Integer(int);
        }

        Value::Decimal(
            value
                .as_f64()
                .expect("a numeric value that is not an integer must be a float"),
        )
    }
}

impl<T: Into<Value>> From<Vec<T>> for Value {
    fn from(value: Vec<T>) -> Self {
        Value::Array(value.into_iter().map(Into::into).collect())
    }
}

impl<K: ToString, V: Into<Value>> From<hcl::Object<K, V>> for Value {
    fn from(value: hcl::Object<K, V>) -> Self {
        Value::Object(
            value
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.into()))
                .collect(),
        )
    }
}

impl From<hcl::Value> for Value {
    fn from(value: hcl::Value) -> Value {
        match value {
            hcl::Value::Bool(b) => b.into(),
            hcl::Value::Number(n) => n.into(),
            hcl::Value::String(s) => s.into(),
            hcl::Value::Array(a) => a.into(),
            hcl::Value::Object(o) => o.into(),
            hcl::Value::Null => {
                // FIXME: We assume that we never hit `null`
                panic!("null value found. This should never happen. Please report this.")
            }
        }
    }
}

impl serde::ser::Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Value::Boolean(value) => serializer.serialize_bool(*value),
            Value::Integer(value) => serializer.serialize_i64(*value),
            Value::Decimal(value) => serializer.serialize_f64(*value),
            Value::String(value) => serializer.serialize_str(value),
            Value::Array(value) => {
                let mut ser = serializer.serialize_seq(Some(value.len()))?;
                for element in value {
                    ser.serialize_element(element)?;
                }
                ser.end()
            }
            Value::Object(value) => {
                let mut ser = serializer.serialize_map(Some(value.len()))?;
                for (element_key, element_value) in value {
                    ser.serialize_entry(element_key, element_value)?;
                }
                ser.end()
            }
        }
    }
}
