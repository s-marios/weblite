use crate::descriptions::{self, Schema, TypedSchema};
use serde::Deserialize;
use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};

#[repr(u8)]
#[derive(Deserialize, Debug)]
enum NumberSize {
    Zero = 0,
    One = 1,
    Two = 2,
    Three = 3,
    Four = 4,
}

#[derive(Deserialize, Debug)]
pub struct Entries {
    entries: Vec<Entry>,
}

#[derive(Deserialize, Debug)]
pub struct Entry {
    eoj: String,
    ai: Vec<PropertyInfo>,
}

#[derive(Deserialize, Debug)]
pub struct PropertyInfo {
    pub epc: String,
    pub info: AdditionalInfo,
}

#[derive(Deserialize, Debug)]
pub struct NamedInfo {
    pub property: String,
    pub info: Option<AdditionalInfo>,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum AdditionalInfo {
    Boolean {
        true_value: String,
        false_value: String,
    },
    String,
    Number {
        #[serde(default = "AdditionalInfo::default_size")]
        size: Option<u8>,
        #[serde(default = "AdditionalInfo::default_min")]
        min: Option<f32>,
        max: Option<f32>,
        #[serde(default = "AdditionalInfo::default_multiple_of")]
        #[serde(rename = "multipleOf")]
        multiple_of: Option<f32>,
    },
    Null {
        edt: String,
    },

    Object {
        order: Vec<NamedInfo>,
    },
    Array,
    OneOf,
}

impl AdditionalInfo {
    fn default_size() -> Option<u8> {
        Some(1)
    }

    fn default_min() -> Option<f32> {
        Some(0.0)
    }

    fn default_multiple_of() -> Option<f32> {
        Some(1.0)
    }
}

#[derive(Deserialize, Debug)]
pub struct PropertyValue<T> {
    pub value: T,
    pub edt: String,
}

impl<T> TryFrom<descriptions::PropertyValue<T>> for PropertyValue<T> {
    type Error = String;
    fn try_from(dp: descriptions::PropertyValue<T>) -> std::result::Result<Self, String> {
        if dp.edt.is_none() {
            Err(String::from("No edt value in property value"))
        } else {
            Ok(PropertyValue {
                value: dp.value,
                edt: dp.edt.unwrap(),
            })
        }
    }
}

#[derive(Deserialize, Debug)]
pub enum Converter {
    Boolean {
        true_value: String,
        false_value: String,
    },
    Time {
        format: String,
    },
    Passthrough,
    Enumlist {
        enumlist: Vec<String>,
        values: Vec<PropertyValue<String>>,
    },
    Number {
        minimum: f32,
        maximum: f32,
        #[serde(rename = "multipleOf")]
        multiple_of: f32,
    },
    Null {
        edt: String,
    },
    Object {
        properties: HashMap<String, Converter>,
    },
    Array {
        #[serde(rename = "minItems")]
        min_items: Option<u32>,
        #[serde(rename = "maxItems")]
        max_items: Option<u32>,
        items: Box<Converter>,
    },
    OneOf {
        opts: Vec<Converter>,
    },
}

impl TryFrom<Schema> for Converter {
    type Error = String;

    //this is going to be big...
    fn try_from(schema: Schema) -> std::result::Result<Self, String> {
        match schema {
            //String schema => Time converter
            Schema::T(TypedSchema::String {
                format: Some(format),
                enumlist: _,
                values: _,
            }) => Ok(Converter::Time { format }),
            Schema::T(TypedSchema::String {
                format: None,
                enumlist: None,
                values: None,
            }) => Ok(Converter::Passthrough),
            Schema::T(TypedSchema::String {
                format: None,
                enumlist: Some(_),
                values: None,
            }) => Err("String Schema: enumlist without values!".to_string()),
            //String-enums & time
            Schema::T(TypedSchema::String {
                format: None,
                enumlist,
                values: Some(values),
            }) => {
                if let Ok(vres) = values
                    //we're dealing with an enumlist
                    .into_iter()
                    .map(|p| p.try_into())
                    .collect::<Result<Vec<PropertyValue<String>>, String>>()
                {
                    let enumlist = enumlist.unwrap_or_else(|| {
                        vres.iter()
                            .map(|property| property.value.clone())
                            .collect::<Vec<String>>()
                    });
                    Ok(Converter::Enumlist {
                        enumlist,
                        values: vres,
                    })
                } else {
                    Err(String::from("Enumlist: not all values had edt"))
                }
            }
            //a simple one
            Schema::T(TypedSchema::Null { edt: Some(edt) }) => Ok(Converter::Null { edt }),
            Schema::T(TypedSchema::Null { edt: None }) => {
                Err("Null Schema without edt".to_string())
            }
            Schema::T(TypedSchema::Boolean { values }) => {
                //find the two true/false values, that's all we care

                let true_value = values
                    .iter()
                    .find(|v| v.value && v.edt.is_some())
                    .cloned()
                    .map(|v| v.edt.unwrap())
                    .ok_or_else(|| "No true value".to_string())?;
                let false_value = values
                    .iter()
                    .find(|v| !v.value && v.edt.is_some())
                    .cloned()
                    .map(|v| v.edt.unwrap())
                    .ok_or_else(|| "No false value".to_string())?;
                Ok(Converter::Boolean {
                    true_value,
                    false_value,
                })
            }
            Schema::T(TypedSchema::Number {
                unit: _,
                minimum: Some(minimum),
                maximum: Some(maximum),
                multiple_of,
            }) => Ok(Converter::Number {
                minimum,
                maximum,
                multiple_of: multiple_of.unwrap_or(1f32),
            }),
            Schema::T(TypedSchema::Number {
                unit: _,
                minimum: _,
                maximum: _,
                multiple_of: _,
            }) => Err("Number schema: min and/or max missing".to_string()),
            Schema::T(TypedSchema::Object { properties: _ }) => {
                //this is simple, we don't have ordering info
                //so we just fail
                Err("Object Schema: no ordering information".to_string())
            }
            Schema::T(TypedSchema::Array {
                min_items: _,
                max_items: _,
                items: _,
            }) => {
                //TODO
                Err("Schema Array: unimplemented!".to_string())
            }
            Schema::OneOf(opts) => Ok(Converter::OneOf {
                //try to convert all the schemas to converters
                opts: opts
                    .one_of
                    .into_iter()
                    .map(Converter::try_from)
                    .collect::<Result<Vec<Converter>, String>>()
                    .map_err(|_| "Schema OneOf: cannot convert all schemes".to_string())?,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptions::{self, read_def, Schema, TypedSchema};

    #[test]
    fn ai_api() {
        println!("test!!!");
        let ais: Entries = descriptions::read_def_generic("./tests/ai/test.json").unwrap();
        assert_eq!(ais.entries[0].eoj, "0290");
        assert_eq!(ais.entries[1].ai.is_empty(), true);
    }

    #[test]
    fn number_conversion_ok() {
        let schema = Schema::T(TypedSchema::Number {
            unit: None,
            multiple_of: None,
            minimum: Some(0.1),
            maximum: Some(10.),
        });
        let converter: Converter = schema.try_into().expect("conversion failed!");
        match converter {
            Converter::Number {
                minimum,
                maximum,
                multiple_of,
            } => {
                assert!((minimum - 0.1).abs() < 0.0001);
                assert!((maximum - 10.).abs() < 0.0001);
                assert!((multiple_of - 1.).abs() < 0.0001);
            }
            _ => panic!("should match a number converter!"),
        }
    }

    #[test]
    fn boolean_conversion_ok() {
        let v1 = descriptions::PropertyValue {
            value: true,
            descriptions: descriptions::TextDescription {
                ja: String::from("dont care"),
                en: String::from("dont care in english"),
            },
            edt: Some(String::from("edt_true")),
        };
        let v2 = descriptions::PropertyValue {
            value: false,
            descriptions: descriptions::TextDescription {
                ja: String::from("dont care"),
                en: String::from("dont care in english"),
            },
            edt: Some(String::from("edt_false")),
        };
        let values = vec![v1, v2];
        let schema = Schema::T(TypedSchema::Boolean { values });
        let converter: Converter = schema.try_into().expect("conversion failed!");
        match converter {
            Converter::Boolean {
                true_value,
                false_value,
            } => {
                assert_eq!(true_value, "edt_true");
                assert_eq!(false_value, "edt_false");
            }
            _ => panic!("should match a boolean converter!"),
        }
    }

    #[test]
    fn boolean_conversion_fails_with_no_false_value() {
        let v1 = descriptions::PropertyValue {
            value: true,
            descriptions: descriptions::TextDescription {
                ja: String::from("dont care"),
                en: String::from("dont care in english"),
            },
            edt: Some(String::from("edt_true")),
        };
        let values = vec![v1];
        let schema = Schema::T(TypedSchema::Boolean { values });
        let converter: Result<Converter, String> = schema.try_into();
        assert!(converter.is_err());
        assert_eq!(converter.unwrap_err(), "No false value");
    }
}
