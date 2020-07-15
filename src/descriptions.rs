use serde::Deserialize;
use serde_json::Value;
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
pub enum Schema {
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
        properties: HashMap<String, Value>,
    },
    //TODO OneOf, Array
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
            Schema::Number {
                unit,
                minimum,
                maximum,
                multiple_of,
            } => {
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
            Schema::String {
                format,
                enumlist,
                values,
            } => {
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
            Schema::Boolean { values } => {
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
            Schema::String {
                format,
                enumlist: _,
                values: _,
            } => {
                assert_eq!(format.as_ref().unwrap(), "date");
            }
            _ => panic!("unexpected schema!"),
        }

        //find a "date-time" string def and make sure it's ok
        match &common.properties["currentDateAndTime"].schema {
            Schema::String {
                format,
                enumlist: _,
                values: _,
            } => {
                assert_eq!(format.as_ref().unwrap(), "date-time");
            }
            _ => panic!("unexpected schema!"),
        }
    }
}
