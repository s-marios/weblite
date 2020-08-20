use crate::descriptions::{self, Schema, TypedSchema};
use crate::hex::bytes_as_u32 as accumulate;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::convert::{TryFrom, TryInto};
use std::path::Path;

pub(crate) fn read_ais<P: AsRef<Path>>(filename: P) -> std::io::Result<Entries> {
    descriptions::read_def_generic::<P, Entries>(filename)
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

//impl <'a>BinaryContext<'a> {
//}

impl Converter {
    pub fn convert_json(&self, value: serde_json::Value) -> Result<Vec<u8>, String> {
        match self {
            Converter::Number {
                minimum,
                maximum,
                multiple_of,
            } => Converter::convert_json_number(value, *minimum, *maximum, *multiple_of),
            _ => unimplemented!(),
        }
    }

    fn convert_json_number(
        value: serde_json::Value,
        min: f32,
        max: f32,
        mof: f32,
    ) -> Result<Vec<u8>, String> {
        //TODO figure out Value::Number and to what it'd better be mapped
        //hopefully this will not bite me
        if let Some(num) = value.as_f64() {
            let range = Converter::range_calculate(min, max, mof);
            //TODO this bites me, maybe keep everything as f64?
            //let num = num as f32
            let binary = (((num - min as f64) / mof as f64).round() as u32).to_be_bytes();
            Ok(binary[4 - range..].to_owned())
        } else {
            Err("serde_json_number: couldn't get number as f64!".to_string())
        }
    }

    pub fn convert_binary<'a, 's>(
        &self,
        context: &'a mut BinaryContext<'s>,
    ) -> Result<serde_json::Value, String> {
        match self {
            Converter::Number {
                minimum: _,
                maximum: _,
                multiple_of: _,
            } => self.convert_binary_number(context),
            _ => unimplemented!("not yet!"),
        }
    }

    fn convert_binary_number<'a, 's>(
        &self,
        context: &'a mut BinaryContext<'s>,
    ) -> Result<serde_json::Value, String> {
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
            let value = accumulate(data) as f32;
            //massage it..
            let value = (value * *mof) + min;
            //TODO handle above max?
            if value > *max {
                println!("TODO: we've generated a value greater than max");
            }
            Ok(serde_json::from_str(&value.to_string())
                .map_err(|err| format!("converter: serde json error: {}", err))?)
        } else {
            panic!("this can never happen!")
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
        let ais: Entries = descriptions::read_def_generic("./tests/ai/test.json").unwrap();
        assert_eq!(ais.entries[0].eoj, "0290");
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
        let ais: Entries = descriptions::read_def_generic("./tests/ai/test.json").unwrap();
        let ldd = descriptions::read_def("./tests/dd/generalLighting_v100.json").unwrap();
        //get "rgb" property
        let color = &ldd.properties["rgb"];
        //get rgb additional info
        let rgb_entry = &ais
            .entries
            .iter()
            .find(|entry| entry.eoj == "0290")
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
                assert!((num64 - 7.).abs() < 0.00001)
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
        let bytes = b"\xff\xff\xff\xff"; //this is 17, the second byte should not matter
        let conv = Converter::Number {
            minimum: 25.6,
            maximum: 51.1,
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
                //since minimum is -10 and our raw value is 17
                //this should be seven...
                assert!((num64 - 51.1).abs() < 0.00001)
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

    #[ignore]
    #[test]
    //TODO accuracy bites me!!!
    fn convert_16843009_to_4_bytes_with_translation_ok() {
        let value = json!(16843.009);
        let result = Converter::convert_json_number(value, 0.0, 36843.009, 0.001).unwrap();
        assert_eq!(result.len(), 4);
        assert_eq!(result, b"\x01\x01\x01\x01");
    }
}
