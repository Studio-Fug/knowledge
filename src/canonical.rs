//! v1 JSON: sorted object keys, UTF-8 strings, integers only, no duplicate keys.
use crate::{Error, Result};
use serde::{
    Deserialize, Serialize,
    de::{self, MapAccess, SeqAccess, Visitor},
};
use serde_json::{Map, Number, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fmt};

pub const MAX_BYTES: usize = 4 * 1024 * 1024;

struct Unique<const FLOATS: bool>(Value);

impl<'de, const FLOATS: bool> Deserialize<'de> for Unique<FLOATS> {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        struct JsonVisitor<const FLOATS: bool>;
        impl<'de, const FLOATS: bool> Visitor<'de> for JsonVisitor<FLOATS> {
            type Value = Unique<FLOATS>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("unambiguous integer-only JSON")
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> std::result::Result<Unique<FLOATS>, E> {
                Ok(Unique(Value::Bool(v)))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> std::result::Result<Unique<FLOATS>, E> {
                Ok(Unique(Value::Number(Number::from(v))))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> std::result::Result<Unique<FLOATS>, E> {
                Ok(Unique(Value::Number(Number::from(v))))
            }
            fn visit_f64<E: de::Error>(self, value: f64) -> std::result::Result<Unique<FLOATS>, E> {
                if FLOATS {
                    Number::from_f64(value)
                        .map(|n| Unique(Value::Number(n)))
                        .ok_or_else(|| E::custom("non-finite numbers are unsupported"))
                } else {
                    Err(E::custom("floating-point numbers are unsupported"))
                }
            }
            fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<Unique<FLOATS>, E> {
                Ok(Unique(Value::String(v.into())))
            }
            fn visit_string<E: de::Error>(
                self,
                v: String,
            ) -> std::result::Result<Unique<FLOATS>, E> {
                Ok(Unique(Value::String(v)))
            }
            fn visit_unit<E: de::Error>(self) -> std::result::Result<Unique<FLOATS>, E> {
                Ok(Unique(Value::Null))
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> std::result::Result<Unique<FLOATS>, A::Error> {
                let mut values = Vec::new();
                while let Some(Unique(value)) = seq.next_element::<Unique<FLOATS>>()? {
                    values.push(value);
                }
                Ok(Unique(Value::Array(values)))
            }
            fn visit_map<A: MapAccess<'de>>(
                self,
                mut map: A,
            ) -> std::result::Result<Unique<FLOATS>, A::Error> {
                let mut seen = BTreeSet::new();
                let mut values = Map::new();
                while let Some((key, Unique(value))) = map.next_entry::<String, Unique<FLOATS>>()? {
                    if !seen.insert(key.clone()) {
                        return Err(de::Error::custom("duplicate object key"));
                    }
                    values.insert(key, value);
                }
                Ok(Unique(Value::Object(values)))
            }
        }
        deserializer.deserialize_any(JsonVisitor::<FLOATS>)
    }
}

pub fn parse<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    if bytes.len() > MAX_BYTES {
        return Err(Error::new("too_large", "artifact or request exceeds 4 MiB"));
    }
    let Unique(value): Unique<false> = serde_json::from_slice(bytes)?;
    Ok(serde_json::from_value(value)?)
}

pub fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    fn write(v: &Value, out: &mut Vec<u8>) -> Result<()> {
        match v {
            Value::Object(map) => {
                out.push(b'{');
                let mut entries: Vec<_> = map.iter().collect();
                entries.sort_by(|a, b| a.0.cmp(b.0));
                for (i, (key, value)) in entries.into_iter().enumerate() {
                    if i != 0 {
                        out.push(b',');
                    }
                    serde_json::to_writer(&mut *out, key)?;
                    out.push(b':');
                    write(value, out)?;
                }
                out.push(b'}');
            }
            Value::Array(items) => {
                out.push(b'[');
                for (i, item) in items.iter().enumerate() {
                    if i != 0 {
                        out.push(b',');
                    }
                    write(item, out)?;
                }
                out.push(b']');
            }
            Value::Number(n) if !n.is_i64() && !n.is_u64() => {
                return Err(Error::new(
                    "invalid_json",
                    "floating-point numbers are unsupported",
                ));
            }
            _ => serde_json::to_writer(out, v)?,
        }
        Ok(())
    }
    let mut bytes = Vec::new();
    write(&serde_json::to_value(value)?, &mut bytes)?;
    if bytes.len() > MAX_BYTES {
        return Err(Error::new("too_large", "canonical content exceeds 4 MiB"));
    }
    Ok(bytes)
}

pub fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub fn is_hex(value: &str, bytes: usize) -> bool {
    value.len() == bytes * 2
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}

/// API queries may use decimal thresholds; artifact canonicalization stays integer-only.
pub fn parse_request<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    if bytes.len() > MAX_BYTES {
        return Err(Error::new("too_large", "request exceeds 4 MiB"));
    }
    let Unique(value): Unique<true> = serde_json::from_slice(bytes)?;
    Ok(serde_json::from_value(value)?)
}

pub fn encode_response<T: Serialize>(value: &T) -> Result<Vec<u8>> {
    let bytes = serde_json::to_vec(value)?;
    if bytes.len() > MAX_BYTES {
        return Err(Error::new("too_large", "response exceeds 4 MiB"));
    }
    Ok(bytes)
}
