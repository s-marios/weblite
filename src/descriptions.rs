use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Debug)]
pub struct TextDescription {
    pub ja: String,
    pub en: String,
}

#[derive(Deserialize, Debug)]
pub struct DeviceProperty {
    pub epc: String,
    pub descriptions: TextDescription,
    pub writable: bool,
    pub observable: bool,
    pub schema: Schema,
}

#[derive(Deserialize, Debug)]
pub struct PropertyValue<T> {
    pub value: T,
    pub descriptions: TextDescription,
    pub edt: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum TypedSchema {
    Boolean {
        values: Vec<PropertyValue<bool>>,
    },
    String {
        format: Option<String>,
        #[serde(rename = "enum")]
        enumlist: Option<Vec<String>>,
        values: Option<Vec<PropertyValue<String>>>,
    },
    Number {
        unit: Option<String>,
        minimum: Option<f32>,
        maximum: Option<f32>,
        #[serde(rename = "multipleOf")]
        multiple_of: Option<f32>,
    },
    Null {
        edt: String,
    },
    Object {
        properties: HashMap<String, Schema>,
    },
    Array {
        #[serde(rename = "minItems")]
        min_items: Option<u8>,
        #[serde(rename = "maxItems")]
        max_items: Option<u8>,
        items: Box<Schema>,
    },
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub struct Options {
    #[serde(rename = "oneOf")]
    one_of: Vec<Schema>,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum Schema {
    T(TypedSchema),
    OneOf(Options),
}
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DeviceDescription {
    pub device_type: String,
    pub eoj: String,
    pub descriptions: TextDescription,
    pub properties: HashMap<String, DeviceProperty>,
}

pub fn read_def(filename: &str) -> std::io::Result<DeviceDescription> {
    let dd_string = std::fs::read_to_string(filename)?;
    let dd: DeviceDescription = serde_json::from_str(&dd_string)?;
    Ok(dd)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }

    #[test]
    fn read_mono_functional_lighting_device_description_v110() {
        let light = read_def("tests/dd/monoFunctionalLighting_v110.json").unwrap();
        assert_eq!(light.device_type, "monoFunctionalLighting");
        assert_eq!(light.eoj, "0x0291");
        assert_eq!(light.descriptions.en, "mono functional lighting");
        assert_eq!(light.properties["brightness"].epc, "0xB0");
        assert_eq!(
            light.properties["brightness"].descriptions.en,
            "Illuminance level"
        );
        assert_eq!(
            light.properties["brightness"].descriptions.ja,
            "照度レベル設定"
        );
        assert_eq!(light.properties["brightness"].writable, true);
        assert_eq!(light.properties["brightness"].observable, false);
        match &light.properties["brightness"].schema {
            Schema::T(TypedSchema::Number {
                unit,
                minimum,
                maximum,
                multiple_of,
            }) => {
                assert_eq!(unit.as_ref().unwrap(), "%");
                assert_eq!(*minimum, Some(0f32));
                assert_eq!(*maximum, Some(100f32));
            }
            _ => panic!("schema is not a number!!!"),
        }
    }

    #[test]
    fn read_general_lighting_v100() {
        let light = read_def("tests/dd/generalLighting_v100.json").unwrap();
        assert_eq!(light.device_type, "generalLighting");
        assert_eq!(light.properties["brightness"].writable, true);
        match &light.properties["operationMode"].schema {
            Schema::T(TypedSchema::String {
                format,
                enumlist,
                values,
            }) => {
                assert_eq!(format.is_none(), true);
                assert_eq!(enumlist.as_ref().unwrap().len(), 4);
                assert_eq!(enumlist.as_ref().unwrap()[2], "night");
                assert_eq!(values.as_ref().unwrap().len(), 4);
                assert_eq!(values.as_ref().unwrap()[0].value, "auto");
                assert_eq!(values.as_ref().unwrap()[1].value, "normal");
                assert_eq!(values.as_ref().unwrap()[2].value, "night");
                assert_eq!(values.as_ref().unwrap()[3].value, "color");
                assert_eq!(values.as_ref().unwrap()[0].edt.as_ref().unwrap(), "0x41");
                assert_eq!(values.as_ref().unwrap()[1].edt.as_ref().unwrap(), "0x42");
                assert_eq!(values.as_ref().unwrap()[2].edt.as_ref().unwrap(), "0x43");
                assert_eq!(values.as_ref().unwrap()[3].edt.as_ref().unwrap(), "0x45");
            }
            _ => panic!("unexpected schema!"),
        }
    }

    #[test]
    fn read_common_items_v110() {
        let common = read_def("tests/dd/commonItems_v110.json").unwrap();
        //check a boolean field
        match &common.properties["powerSaving"].schema {
            Schema::T(TypedSchema::Boolean { values }) => {
                assert_eq!(values.len(), 2);
                assert_eq!(values[0].value, true);
                assert_eq!(values[1].value, false);
                assert_eq!(values[0].edt.as_ref().unwrap(), "0x41");
                assert_eq!(values[1].edt.as_ref().unwrap(), "0x42");
                assert_eq!(values[0].descriptions.en, "Power saving operation");
                assert_eq!(values[1].descriptions.en, "Normal operation");
            }
            _ => panic!("unexpected schema!"),
        };
        //find a "date" string def and make sure it's ok
        match &common.properties["productionDate"].schema {
            Schema::T(TypedSchema::String {
                format,
                enumlist: _,
                values: _,
            }) => {
                assert_eq!(format.as_ref().unwrap(), "date");
            }
            _ => panic!("unexpected schema!"),
        }

        //find a "date-time" string def and make sure it's ok
        match &common.properties["currentDateAndTime"].schema {
            Schema::T(TypedSchema::String {
                format,
                enumlist: _,
                values: _,
            }) => {
                assert_eq!(format.as_ref().unwrap(), "date-time");
            }
            _ => panic!("unexpected schema!"),
        }
    }

    #[test]
    fn read_floor_heater_v110_array_and_oneof_schema() {
        let heater = read_def("tests/dd/floorHeater_v110.json").unwrap();
        //check the contents of an array schema
        match &heater.properties["controllableZone"].schema {
            Schema::T(TypedSchema::Array {
                min_items,
                max_items,
                items,
            }) => {
                //what the ...
                match &**items {
                    Schema::T(TypedSchema::Boolean { values }) => {
                        assert_eq!(values[0].value, true);
                        assert_eq!(values[1].value, false);
                    }
                    _ => panic!("unexpected array item!"),
                }
            }
            _ => panic!("unexpected schema!"),
        };

        //check the contents of an "oneOf" schema
        match &heater.properties["targetTemperature1"].schema {
            Schema::OneOf(opts) => {
                assert_eq!(opts.one_of.len(), 2);
                match &opts.one_of[0] {
                    Schema::T(TypedSchema::Number {
                        unit,
                        minimum,
                        maximum,
                        multiple_of,
                    }) => {
                        let error = 0.0001;
                        assert_eq!(unit.as_ref().unwrap(), "Celsius");
                        assert!((minimum.unwrap() - 0f32) < error);
                        assert!((maximum.unwrap() - 50f32) < error);
                        assert!((multiple_of.unwrap() - 1f32) < error);
                    }
                    _ => panic!("unexpected option!"),
                };
                match &opts.one_of[1] {
                    Schema::T(TypedSchema::String {
                        format,
                        enumlist,
                        values,
                    }) => {
                        assert_eq!(enumlist.as_ref().unwrap().len(), 1);
                        assert_eq!(enumlist.as_ref().unwrap()[0], "auto");
                        assert_eq!(values.as_ref().unwrap()[0].edt.as_ref().unwrap(), "0x41");
                    }
                    _ => panic!("unexpected option!"),
                }
            }
            _ => panic!("unexpected schema!"),
        }
    }
}
