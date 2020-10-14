use crate::descriptions::{self, Schema, TypedSchema};
use crate::hex::{self, bytes_as_u32};
use crate::NetError;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::convert::{TryFrom, TryInto};
use std::path::Path;

pub(crate) fn read_ais<P: AsRef<Path>>(filename: P) -> std::io::Result<Ais> {
    println!("reading additional information: {}", filename.as_ref().display());
    descriptions::read_def_generic::<P, Ais>(filename)
}

/// All the Additional Information entries available
#[derive(Deserialize, Debug)]
pub struct Ais {
    entries: Vec<Entry>,
}

impl Ais {
    pub fn properties(&self, eoj: &str) -> Option<&Vec<PropertyInfo>> {
        Some(&self.entries.iter().find(|entry| entry.eoj == eoj)?.ai)
    }
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
    pub name: String,
    pub info: Option<AdditionalInfo>,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum AdditionalInfo {
    Boolean {
        true_value: String,
        false_value: String,
    },
    //TODO
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

#[derive(Deserialize, Clone, Debug)]
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

#[derive(Deserialize, Clone, Debug)]
pub enum Converter {
    Boolean {
        true_value: String,
        false_value: String,
    },
    Time {
        format: String,
    },
    Passthrough,
    //hashmap is <enum-name, edt>
    Enumlist(HashMap<String, String>),
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
        properties: Vec<(String, Converter)>,
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

pub struct BinaryContext<'a> {
    data: &'a [u8],
}

impl<'a> From<&'a [u8]> for BinaryContext<'a> {
    fn from(data: &'a [u8]) -> Self {
        BinaryContext { data }
    }
}

impl Converter {
    pub fn convert_json(&self, value: serde_json::Value) -> Result<Vec<u8>, NetError> {
        match self {
            Converter::Number {
                minimum,
                maximum,
                multiple_of,
            } => Converter::convert_json_number(value, *minimum, *maximum, *multiple_of),
            Converter::Object { properties } => Converter::convert_json_object(value, properties),
            Converter::Passthrough => Converter::json_passthrough(
                value
                    .as_str()
                    .ok_or_else(|| NetError::BadRequest("input not a string!".to_string()))?,
            ),
            Converter::Enumlist(map) => Converter::json_enumlist(
                value
                    .as_str()
                    .ok_or_else(|| NetError::BadRequest("input not a string!".to_string()))?,
                map,
            ),
            Converter::Boolean {
                true_value,
                false_value,
            } => Converter::json_boolean(value, true_value, false_value),
            _ => Err(NetError::Internal(format!(
                "unimplemented conversion for: {}",
                value
            ))),
        }
    }

    pub fn convert_binary<'a, 's>(
        &self,
        context: &'a mut BinaryContext<'s>,
    ) -> Result<serde_json::Value, NetError> {
        match self {
            Converter::Number {
                minimum: _,
                maximum: _,
                multiple_of: _,
            } => self.convert_binary_number(context),
            Converter::Object { properties: _ } => self.convert_binary_object(context),
            Converter::Passthrough => Converter::binary_passthrough(context),
            Converter::Enumlist(map) => Converter::binary_enumlist(context, map),
            Converter::Boolean {
                true_value,
                false_value,
            } => Converter::binary_boolean(context, true_value, false_value),
            _ => unimplemented!("not yet!"),
        }
    }

    fn convert_json_number(
        value: serde_json::Value,
        min: f32,
        max: f32,
        mof: f32,
    ) -> Result<Vec<u8>, NetError> {
        //TODO figure out Value::Number and to what it'd better be mapped
        //hopefully this will not bite me
        if let Some(num) = value.as_f64() {
            Converter::check_in_range(num as f32, min, max)?;
            let range = Converter::range_calculate(min, max, mof);
            //TODO this bites me, maybe keep everything as f64?
            //let num = num as f32
            let binary = ((num as f64 / mof as f64).round() as u32).to_be_bytes();
            Ok(binary[4 - range..].to_owned())
        } else {
            Err(NetError::BadRequest(
                "couldn't get number as f64!".to_string(),
            ))
        }
    }

    fn check_in_range(val: f32, min: f32, max: f32) -> Result<(), NetError> {
        if val < min {
            Err(NetError::Range("less than min".to_string()))
        } else if val > max {
            Err(NetError::Range("greater than max".to_string()))
        } else {
            Ok(())
        }
    }

    fn convert_json_object(
        value: serde_json::Value,
        properties: &[(String, Converter)],
    ) -> Result<Vec<u8>, NetError> {
        Ok(properties
            .iter()
            .map(|(name, conv)| {
                conv.convert_json(
                    value
                        .get(name)
                        .ok_or_else(|| {
                            NetError::BadRequest(format!(
                                "required property not present: \'{}\'",
                                name
                            ))
                        })?
                        .clone(),
                )
            })
            .collect::<Result<Vec<Vec<u8>>, NetError>>()?
            .into_iter()
            .flatten()
            .collect())
    }

    fn json_passthrough(input: &str) -> Result<Vec<u8>, NetError> {
        Ok(input.bytes().collect())
    }

    fn json_enumlist(input: &str, map: &HashMap<String, String>) -> Result<Vec<u8>, NetError> {
        Ok(vec![hex::to_usize(
            map.get(input)
                .ok_or_else(|| NetError::BadRequest(format!("no entry for: {}", input)))?,
        )
        .ok_or_else(|| {
            NetError::Internal(format!("json_enum: invalid entry for {}, fix ASAP!", input))
        })? as u8])
    }

    fn json_boolean(
        value: serde_json::Value,
        true_value: &str,
        false_value: &str,
    ) -> Result<Vec<u8>, NetError> {
        Ok(hex::to_bytes(
            if value
                .as_bool()
                .ok_or_else(|| NetError::BadRequest("not a boolean!".to_string()))?
            {
                true_value
            } else {
                false_value
            },
        )
        .ok_or_else(|| {
            NetError::Internal("json_boolean: failed to convert to hex, fix asap!".to_string())
        })?)
    }

    //TODO deal with negative numbers!
    fn convert_binary_number<'a, 's>(
        &self,
        context: &'a mut BinaryContext<'s>,
    ) -> Result<serde_json::Value, NetError> {
        if let Converter::Number {
            minimum: min,
            maximum: max,
            multiple_of: mof,
        } = self
        {
            let size = Converter::range_calculate(*min, *max, *mof);
            //get our data...
            let data = &context.data[..size];
            //...and update context i.e. advance data slice
            context.data = &context.data[size..];
            //make a number from the data

            //TODO look this up, but I dont think we have anything more than an u32
            //TODO worse than that, how do we deal with negative numbers?
            let value = bytes_as_u32(data) as f32;
            //massage it..
            let value = value * *mof;
            //TODO handle above max?
            if value > *max {
                println!("TODO: we've generated a value greater than max");
            }
            Ok(serde_json::from_str(&value.to_string()).map_err(|err| {
                NetError::Conversion(format!("converter: serde json error: {}", err))
            })?)
        } else {
            panic!("this can never happen!")
        }
    }

    fn convert_binary_object<'a, 's>(
        &self,
        context: &'a mut BinaryContext<'s>,
    ) -> Result<serde_json::Value, NetError> {
        if let Converter::Object { properties } = self {
            let mut map = serde_json::map::Map::with_capacity(properties.len());
            //TODO there's some inference problems here, I don't know why
            let _: Result<(), NetError> = properties.iter().try_for_each(|(name, conv)| {
                let conv_res: serde_json::Value = conv.convert_binary(context).map_err(|err| {
                    NetError::Conversion(format!("cbo: sub-converter failed {:?}", err))
                })?;
                map.insert(name.clone(), conv_res);
                Ok(())
            });
            Ok(serde_json::Value::Object(map))
        } else {
            panic!("this can never happen!")
        }
    }

    fn binary_passthrough<'s>(
        context: &mut BinaryContext<'s>,
    ) -> Result<serde_json::Value, NetError> {
        //get our data out
        let data = &context.data[..];
        //consume the whole thing, advance context
        context.data = &context.data[data.len()..];
        Ok(serde_json::Value::String(
            String::from_utf8_lossy(&data).to_string(),
        ))
    }

    fn binary_enumlist<'s>(
        context: &mut BinaryContext<'s>,
        map: &HashMap<String, String>,
    ) -> Result<serde_json::Value, NetError> {
        //now this is interesting..
        //do we have enums that are more than one byte?
        //TODO check above

        //our data as a string..
        let data_string = format!("0x{:2x}", context.data[0]);
        println!("datastring: {}", data_string);
        //advance context
        context.data = &context.data[1..];
        //..just go through the map and find the thing
        Ok(serde_json::Value::String(
            map.iter()
                .find(|(_, v)| *v == &data_string)
                .ok_or_else(|| {
                    NetError::Conversion(format!(
                        "binary_enumlist: no key for edt: {}",
                        data_string
                    ))
                })?
                .0
                .clone(),
        ))
    }

    fn binary_boolean<'s>(
        context: &mut BinaryContext<'s>,
        true_value: &str,
        false_value: &str,
    ) -> Result<serde_json::Value, NetError> {
        //get our value
        let val = context.data.get(0).ok_or(NetError::WriteNG)?;
        //advance
        context.data = &context.data[1..];

        //Ok this is getting stupid
        let tv =
            hex::to_bytes(true_value).ok_or_else(|| NetError::Internal("bad data".to_string()))?[0];
        let fv = hex::to_bytes(false_value)
            .ok_or_else(|| NetError::Internal("bad data".to_string()))?[0];

        if *val == tv {
            Ok(serde_json::Value::Bool(true))
        } else if *val == fv {
            Ok(serde_json::Value::Bool(false))
        } else {
            Err(NetError::Conversion(format!(
                "binary_boolean: Invalid binary value: {}",
                val
            )))
        }
    }

    fn range_calculate(min: f32, max: f32, mof: f32) -> usize {
        assert!(!(mof.abs() < 0.00001));
        let range = (max - min) / mof;
        if range < 255.0001 {
            1
        } else if range < 65536.001 {
            2
        } else if range < 16777216.1 {
            3
        } else {
            4
        }
    }
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
                //TODO we are ignoring the enumlist completely
                //and rely on the values themselves, is this ok?
                enumlist: _,
                values: Some(values),
            }) => {
                if let Ok(vres) = values
                    //we're dealing with an enumlist
                    .into_iter()
                    .map(|p| p.try_into())
                    .collect::<Result<Vec<PropertyValue<String>>, String>>()
                {
                    Ok(Converter::Enumlist(
                        vres.into_iter()
                            .map(|property| (property.value, property.edt))
                            .collect::<HashMap<String, String>>(),
                    ))
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

pub fn fuse(description: &Schema, ai: &AdditionalInfo) -> Result<Converter, String> {
    //smash bits together..
    //this is horrible due to the matching of enum variants...
    match (description, ai) {
        (Schema::T(TypedSchema::Object { properties }), AdditionalInfo::Object { order }) => {
            fuse_objects(properties, order)
        }
        //the matching is horrible
        (
            Schema::T(TypedSchema::Number {
                unit: _,
                minimum: smin,
                maximum: smax,
                multiple_of: smof,
            }),
            AdditionalInfo::Number {
                size: _,
                min: amin,
                max: amax,
                multiple_of: amof,
            },
        ) => {
            let min = smin
                .or(*amin)
                .ok_or_else(|| "fuse numbers: no min spec!".to_string())?;
            let max = smax
                .or(*amax)
                .ok_or_else(|| "fuse numbers: no max spec!".to_string())?;
            let mof = smof
                .or(*amof)
                .ok_or_else(|| "fuse numbers: no multipe_of spec!".to_string())?;
            Ok(Converter::Number {
                minimum: min,
                maximum: max,
                multiple_of: mof,
            })
        }
        _ => unimplemented!(),
    }
}

fn fuse_objects(
    properties: &HashMap<String, Schema>,
    order: &[NamedInfo],
) -> Result<Converter, String> {
    if properties.len() != order.len() {
        return Err("fuse objects: ordering info different from properties".to_string());
    }
    //check that all properties are covered
    let key_set = properties.keys().collect::<HashSet<_>>();
    if order.iter().any(|ni| !key_set.contains(&ni.name)) {
        return Err("fuse objects: properties/order mismatch".to_string());
    }
    //everything aligns, try to convert
    //for each thing as specified in the order, try to get a converter
    let converters = order
        .iter()
        .map(|ni| {
            //we know this exists
            let prop_schema = properties.get(&ni.name).unwrap();

            //do we have additional info?
            if ni.info.is_some() {
                //yes, fuse
                fuse(prop_schema, ni.info.as_ref().unwrap())
            } else {
                //no, try to promote to converter
                Converter::try_from(prop_schema.clone())
            }
        })
        .collect::<Result<Vec<Converter>, String>>()
        .map_err(|err| format!("fuse objects: some converter failed: {}", err))?;
    //we have our converters, zip them with names and make a hashmap out of them
    Ok(Converter::Object {
        properties: order
            .iter()
            .map(|ni| ni.name.clone())
            .zip(converters)
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptions::{self, read_def, Schema, TypedSchema};
    use serde_json::json;

    #[test]
    fn ai_api() {
        println!("test!!!");
        let ais: Ais = read_ais("./tests/ai/test.json").unwrap();
        assert_eq!(ais.entries[0].eoj, "0x0290");
        assert_eq!(ais.entries[1].ai.is_empty(), true);
    }

    fn convert_properties(filepath: &str) -> Vec<Result<Converter, String>> {
        read_def(filepath)
            .unwrap()
            .properties
            .into_iter()
            .inspect(|prop| {
                println!(
                    "prop: {} {}, {}, {}",
                    prop.0, prop.1.epc, prop.1.writable, prop.1.observable
                )
            })
            .map(|prop| Converter::try_from(prop.1.schema))
            .inspect(|res| println!("conversion: {:?}", res))
            .collect::<Vec<Result<Converter, String>>>()
    }

    #[test]
    fn try_convert_properties() {
        println!("\ncommon:\n");
        convert_properties("tests/dd/commonItems_v110.json");
        println!("\nlighting:\n");
        convert_properties("tests/dd/generalLighting_v100.json");
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

    #[test]
    fn fuse_objects_ok() {
        let ais: Ais = read_ais("./tests/ai/test.json").unwrap();
        let ldd = descriptions::read_def("./tests/dd/generalLighting_v100.json").unwrap();
        //get "rgb" property
        let color = &ldd.properties["rgb"];
        //get rgb additional info
        let rgb_entry = &ais
            .entries
            .iter()
            .find(|entry| entry.eoj == "0x0290")
            .unwrap()
            .ai[0];

        assert_eq!(rgb_entry.epc, "0xC0");
        let fused = fuse(&color.schema, &rgb_entry.info).expect("failed to fuse rgb!");
        //it fused!! check stuff
        match fused {
            Converter::Object { properties } => {
                assert_eq!(properties.len(), 3);
                properties
                    .iter()
                    .zip(vec!["red", "green", "blue"].iter())
                    .for_each(|((name, conv), exp_name)| {
                        //check name
                        assert_eq!(name, exp_name);
                        //check converter
                        match conv {
                            Converter::Number {
                                minimum,
                                maximum,
                                multiple_of,
                            } => {
                                assert!(minimum.abs() < 0.00001);
                                assert!((maximum - 255.).abs() < 0.00001);
                                assert!((multiple_of - 1.).abs() < 0.00001);
                            }
                            _ => panic!("rgb converter, inner converters not numbers!"),
                        }
                    });
            }
            _ => panic!("should match an object converter!"),
        }
    }

    #[test]
    fn convert_number_from_binary_ok() {
        let bytes = b"\x11\xff"; //this is 17, the second byte should not matter
        let conv = Converter::Number {
            minimum: -10.,
            maximum: 150.,
            multiple_of: 1.,
        };

        let mut context = BinaryContext { data: &bytes[..] };

        let result = conv.convert_binary(&mut context).unwrap();
        match result {
            serde_json::Value::Number(num) => {
                let num64 = num.as_f64().unwrap();
                println!("num is: {}, f64: {}", num, num64);
                //since minimum is -10 and our raw value is 17
                //this should be seven...
                assert!((num64 - 17.).abs() < 0.00001)
            }
            _ => panic!("we were supposed to get a number back!"),
        }

        //did the context advance?
        assert_eq!(context.data.len(), 1);
        assert_eq!(context.data[0], 0xff);
        //whew... everything is fine
    }

    #[test]
    fn convert_number_from_bytes_with_mof_ok() {
        let bytes = b"\xff\xff\xff\xff";
        let conv = Converter::Number {
            minimum: 0.,
            maximum: 25.5,
            multiple_of: 0.1,
        };

        let mut context = BinaryContext { data: &bytes[..] };

        let result = conv.convert_binary(&mut context).unwrap();
        //did the context advance?
        assert_eq!(context.data.len(), 3);
        assert_eq!(context.data[0], 0xff);
        //check the conversion result
        match result {
            serde_json::Value::Number(num) => {
                let num64 = num.as_f64().unwrap();
                println!("num is: {}, f64: {}", num, num64);
                assert!((num64 - 25.5).abs() < 0.00001)
            }
            _ => panic!("we were supposed to get a number back!"),
        }
    }

    #[test]
    fn convert_number_from_4_bytes_ok() {
        let bytes = b"\x01\x01\x01\x01"; //this is 17, the second byte should not matter
        let conv = Converter::Number {
            minimum: 0.,
            maximum: std::u32::MAX as f32,
            multiple_of: 0.1,
        };

        let mut context = BinaryContext { data: &bytes[..] };

        let result = conv.convert_binary(&mut context).unwrap();
        //did the context advance?
        assert_eq!(context.data.len(), 0);
        //check the conversion result
        match result {
            serde_json::Value::Number(num) => {
                let num64 = num.as_f64().unwrap();
                println!("num is: {}, f64: {}", num, num64);
                //since minimum is -10 and our raw value is 17
                //this should be seven...
                assert!((num64 - 1684300.9).abs() < 0.00001)
            }
            _ => panic!("we were supposed to get a number back!"),
        }
    }

    #[test]
    fn convert_number_to_1_byte_ok() {
        let value = json!(255);
        let result = Converter::convert_json_number(value, 0., 255., 1.).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], 255);
    }

    #[test]
    fn convert_number_that_exceeds_max_fails() {
        let value = json!(300);
        match Converter::convert_json_number(value, 0., 255., 1.) {
            Err(NetError::Range(err)) => assert!(err.contains("greater")),
            _ => panic!("expecting range error!"),
        }
    }

    #[test]
    fn convert_number_that_is_less_than_min_fails() {
        let value = json!(-300);
        match Converter::convert_json_number(value, 0., 255., 1.) {
            Err(NetError::Range(err)) => assert!(err.contains("less")),
            _ => panic!("expecting range error!"),
        }
    }

    #[test]
    fn convert_number_after_rounding_to_1_byte_ok() {
        let value = json!(254.9999999);
        let result = Converter::convert_json_number(value, 0., 255., 1.).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], 255);
    }

    #[test]
    fn convert_512_after_rounding_to_2_bytes_ok() {
        let value = json!(511);
        let result = Converter::convert_json_number(value, 0., 1024., 1.).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], 1);
        assert_eq!(result[1], 255);
    }

    #[test]
    fn convert_65793_to_3_bytes_ok() {
        let value = json!(65793);
        let result = Converter::convert_json_number(value, 0., 128000., 1.).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result, b"\x01\x01\x01");
    }

    #[test]
    fn convert_16843009_to_4_bytes_ok() {
        let value = json!(16843009);
        let result = Converter::convert_json_number(value, 0., 36843009., 1.).unwrap();
        assert_eq!(result.len(), 4);
        assert_eq!(result, b"\x01\x01\x01\x01");
    }

    #[test]
    fn convert_2313_to_2_bytes_ok() {
        let value = json!(23.13);
        let result = Converter::convert_json_number(value, -20., 40., 0.01).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result, b"\x09\x09");
    }

    #[test]
    fn convert_temperature_ok() {
        let mut context = b"\x01\x00"[..].into();
        let converter = Converter::Number {
            minimum: -273.2,
            maximum: 3276.6,
            multiple_of: 0.1,
        };
        let result = converter.convert_binary_number(&mut context).unwrap();
        assert_eq!(result.to_string(), "25.6");
    }

    #[ignore]
    #[test]
    //TODO accuracy bites me!!!
    //UPDATE clippy reports excessive accuracy, we may need to switch to f64
    fn convert_16843009_to_4_bytes_with_translation_ok() {
        let value = json!(16843.009);
        let result = Converter::convert_json_number(value, 0.0, 36843.009, 0.001).unwrap();
        assert_eq!(result.len(), 4);
        assert_eq!(result, b"\x01\x01\x01\x01");
    }

    fn make_rgb_conv() -> Converter {
        Converter::Object {
            properties: vec!["red".to_string(), "green".to_string(), "blue".to_string()]
                .into_iter()
                .zip(std::iter::repeat(Converter::Number {
                    minimum: 0.,
                    maximum: 255.,
                    multiple_of: 1.,
                }))
                .collect(),
        }
    }

    #[test]
    fn convert_rgb_to_3_bytes_ok() {
        //first, make the object converter
        let obj_conv = make_rgb_conv();

        //let's make our input
        let value = json!({
            "red": 0,
            "green":128,
            "blue": 255
        });

        let result = obj_conv.convert_json(value).unwrap();
        assert_eq!(result, b"\x00\x80\xff");
    }

    #[test]
    fn convert_rgb_to_3_bytes_exceeds_max_fails() {
        //first, make the object converter
        let obj_conv = make_rgb_conv();

        //let's make our invalid input
        //green exceeds max
        let value = json!({
            "red": 0,
            "green":1280,
            "blue": 255000
        });

        match obj_conv.convert_json(value) {
            Err(NetError::Range(err)) => assert!(err.contains("greater")),
            _ => panic!("expecting range error!"),
        }
    }

    #[test]
    fn convert_rgb_from_3_bytes_ok() {
        //first, make the object converter
        let obj_conv = make_rgb_conv();

        //let's make our input
        //
        ////original
        //let data = b"\x01\x02\x03";
        //let mut context = BinaryContext { data: &data[..] };
        //let value = obj_conv.convert_binary(&mut context).unwrap();
        //
        ////this is horrible
        //let mut data = b"\x01\x02\x03";
        //let value = obj_conv.convert_binary(&mut (&data[..]).into()).unwrap();
        //
        //perfect!
        let mut context = b"\x01\x02\x03"[..].into();
        let value = obj_conv.convert_binary(&mut context).unwrap();

        assert_eq!(value["red"], 1);
        assert_eq!(value["green"], 2);
        assert_eq!(value["blue"], 3);
        println!("value: {}", value);
    }

    #[test]
    fn passthrough_conversions_ok() {
        let vs = "test";
        let data = b"test";

        //value->binary
        let bd = Converter::json_passthrough(vs).unwrap();
        assert_eq!(bd, data);
        let mut context = bd.as_slice().into();
        let vbd = Converter::binary_passthrough(&mut context).unwrap();
        assert_eq!(vbd, vs);
    }

    #[test]
    fn enum_conversions_ok() {
        //value-to-binary
        //let's make our map
        let mut map: HashMap<String, String> = HashMap::new();
        map.insert("test".to_string(), "0x41".to_string());

        let res = Converter::json_enumlist("test", &map).unwrap();
        assert_eq!(res[0], b'A');
        assert_eq!(res.len(), 1);

        //binary-to-value
        let mut context = (&res[..]).into();
        let bres = Converter::binary_enumlist(&mut context, &map).unwrap();
        assert_eq!(bres.as_str().unwrap(), "test");
    }
}
