use serde::Deserialize;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;

#[derive(Deserialize, Debug, Clone)]
pub struct TextDescription {
    pub ja: String,
    pub en: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct DeviceProperty {
    pub epc: String,
    pub descriptions: TextDescription,
    pub writable: bool,
    pub observable: bool,
    pub schema: Schema,
}

#[derive(Deserialize, Debug, Clone)]
pub struct PropertyValue<T> {
    pub value: T,
    pub descriptions: TextDescription,
    pub edt: Option<String>,
}

#[derive(Deserialize, Debug, Clone)]
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
        edt: Option<String>,
    },
    Object {
        properties: HashMap<String, Schema>,
    },
    Array {
        #[serde(rename = "minItems")]
        min_items: Option<u32>,
        #[serde(rename = "maxItems")]
        max_items: Option<u32>,
        items: Box<Schema>,
    },
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub struct Options {
    #[serde(rename = "oneOf")]
    pub one_of: Vec<Schema>,
}

#[derive(Deserialize, Debug, Clone)]
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

pub type Descriptions = Vec<DeviceDescription>;

pub fn read_def<P: AsRef<Path>>(filename: P) -> std::io::Result<DeviceDescription> {
    let dd_string = std::fs::read_to_string(filename)?;
    let dd: DeviceDescription = serde_json::from_str(&dd_string)?;
    Ok(dd)
}

pub fn read_def_generic<P, T>(filename: P) -> std::io::Result<T>
where
    P: AsRef<Path>,
    T: serde::de::DeserializeOwned,
{
    let dd_string = std::fs::read_to_string(filename)?;
    let dd: T = serde_json::from_str(&dd_string)?;
    Ok(dd)
}

pub fn read_device_descriptions<P: AsRef<Path>>(dir: P) -> std::io::Result<Descriptions> {
    let mut dds = vec![];
    for entry in fs::read_dir(dir)?.filter_map(|x| x.ok()) {
        let path = entry.path();
        if path.is_file() && path.extension() == Some(OsStr::new("json")) {
            //we probably have a device description here. Try to read this
            match read_def(&path) {
                Ok(dd) => dds.push(dd),
                Err(error) => println!(
                    "Failed to parse Device Description: {:?}, error {}",
                    path, error
                ),
            }
        }
    }
    Ok(dds)
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
                unit: Some(u),
                minimum: Some(min),
                maximum: Some(max),
                multiple_of: None,
            }) => {
                assert_eq!(u, "%");
                assert!(min.abs() < 0.00001);
                assert!((max - 100f32).abs() < 0.00001);
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
                values: Some(values),
            }) => {
                assert_eq!(format.is_none(), true);
                assert_eq!(enumlist.as_ref().unwrap().len(), 4);
                assert_eq!(enumlist.as_ref().unwrap()[2], "night");
                assert_eq!(values.len(), 4);
                assert_eq!(values[0].value, "auto");
                assert_eq!(values[1].value, "normal");
                assert_eq!(values[2].value, "night");
                assert_eq!(values[3].value, "color");
                assert_eq!(values[0].edt.as_ref().unwrap(), "0x41");
                assert_eq!(values[1].edt.as_ref().unwrap(), "0x42");
                assert_eq!(values[2].edt.as_ref().unwrap(), "0x43");
                assert_eq!(values[3].edt.as_ref().unwrap(), "0x45");
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
                min_items: Some(min),
                max_items: Some(max),
                items,
            }) => {
                assert_eq!(*min, 8);
                assert_eq!(*max, 8);
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
                        format: None,
                        enumlist,
                        values: Some(values),
                    }) => {
                        assert_eq!(enumlist.as_ref().unwrap().len(), 1);
                        assert_eq!(enumlist.as_ref().unwrap()[0], "auto");
                        assert_eq!(values[0].edt.as_ref().unwrap(), "0x41");
                    }
                    _ => panic!("unexpected option!"),
                }
            }
            _ => panic!("unexpected schema!"),
        }
    }

    #[test]
    fn read_temperature_sensor() {
        let sensor = read_def("tests/dd/temperatureSensor_vxxx.json").unwrap();
        assert_eq!(sensor.eoj, "0x0011");
        assert_eq!(sensor.properties["temperature"].epc, "0xE0");
        assert_eq!(sensor.properties["temperature"].writable, false);
        assert_eq!(sensor.properties["temperature"].observable, false);
        match &sensor.properties["temperature"].schema {
            Schema::T(TypedSchema::Number {
                unit,
                minimum,
                maximum,
                multiple_of,
            }) => {
                assert_eq!(unit.as_ref().unwrap(), "Celcius");
                assert!((minimum.unwrap() + 273.2).abs() < 0.00001);
                assert!((maximum.unwrap() - 3276.6).abs() < 0.00001);
                assert!((multiple_of.unwrap() - 0.1).abs() < 0.00001);
            }
            _ => panic!("unexpected schema!"),
        }
    }

    #[test]
    fn read_humidity_sensor() {
        let sensor = read_def("tests/dd/humiditySensor_vxxx.json").unwrap();
        assert_eq!(sensor.eoj, "0x0012");
        assert_eq!(sensor.properties["humidity"].epc, "0xE0");
        assert_eq!(sensor.properties["humidity"].writable, false);
        assert_eq!(sensor.properties["humidity"].observable, false);
        match &sensor.properties["humidity"].schema {
            Schema::T(TypedSchema::Number {
                unit: Some(u),
                minimum: Some(min),
                maximum: Some(max),
                multiple_of,
            }) => {
                assert_eq!(u, "Percentage (%)");
                assert!(min.abs() < 0.00001);
                assert!((max - 100.0).abs() < 0.00001);
                assert_eq!(multiple_of.as_ref(), None);
            }
            _ => panic!("unexpected schema!"),
        }
    }

    #[test]
    fn read_co2_sensor() {
        let sensor = read_def("tests/dd/co2Sensor_vxxx.json").unwrap();
        assert_eq!(sensor.eoj, "0x001B");
        assert_eq!(sensor.properties["co2"].epc, "0xE0");
        assert_eq!(sensor.properties["co2"].writable, false);
        assert_eq!(sensor.properties["co2"].observable, false);
        match &sensor.properties["co2"].schema {
            Schema::T(TypedSchema::Number {
                unit: Some(u),
                minimum: Some(min),
                maximum: Some(max),
                multiple_of: None,
            }) => {
                assert_eq!(u, "ppm");
                assert!(min.abs() < 0.00001);
                assert!(max - 65533.0 < 0.00001);
            }
            _ => panic!("unexpected schema!"),
        }
    }

    #[test]
    fn read_voc_sensor() {
        let sensor = read_def("tests/dd/vocSensor_vxxx.json").unwrap();
        assert_eq!(sensor.eoj, "0x001D");
        assert_eq!(sensor.properties["voc"].epc, "0xE0");
        assert_eq!(sensor.properties["voc"].writable, false);
        assert_eq!(sensor.properties["voc"].observable, false);
        match &sensor.properties["voc"].schema {
            Schema::T(TypedSchema::Number {
                unit: Some(u),
                minimum: Some(min),
                maximum: Some(max),
                multiple_of: None,
            }) => {
                assert_eq!(u, "ppm");
                assert!(min.abs() < 0.00001);
                assert!(max - 65533.0 < 0.00001);
            }
            _ => panic!("unexpected schema!"),
        };

        assert_eq!(sensor.properties["threshold"].epc, "0xB0");
        assert_eq!(sensor.properties["threshold"].writable, true);
        assert_eq!(sensor.properties["threshold"].observable, false);
        match &sensor.properties["threshold"].schema {
            Schema::T(TypedSchema::Number {
                unit: None,
                minimum: Some(min),
                maximum: Some(max),
                multiple_of: None,
            }) => {
                assert!((min - 49.0).abs() < 0.00001);
                assert!((max - 56.0).abs() < 0.00001);
            }
            _ => panic!("unexpected schema!"),
        }

        assert_eq!(sensor.properties["detection"].epc, "0xB1");
        assert_eq!(sensor.properties["detection"].writable, false);
        assert_eq!(sensor.properties["detection"].observable, true);
        match &sensor.properties["detection"].schema {
            Schema::T(TypedSchema::Boolean { values }) => {
                assert_eq!(values.len(), 2);
                assert_eq!(values[0].value, true);
                assert_eq!(values[0].edt.as_ref().unwrap(), "0x41");
                assert_eq!(values[1].value, false);
                assert_eq!(values[1].edt.as_ref().unwrap(), "0x42");
            }
            _ => panic!("unexpected schema!"),
        }
    }

    #[test]
    fn read_human_presence_sensor() {
        let sensor = read_def("tests/dd/humanPresenceSensor_vxxx.json").unwrap();
        assert_eq!(sensor.eoj, "0x0007");

        assert_eq!(sensor.properties["threshold"].epc, "0xB0");
        assert_eq!(sensor.properties["threshold"].writable, true);
        assert_eq!(sensor.properties["threshold"].observable, false);
        match &sensor.properties["threshold"].schema {
            Schema::T(TypedSchema::Number {
                unit: None,
                minimum: Some(min),
                maximum: Some(max),
                multiple_of: None,
            }) => {
                assert!((min - 49.0).abs() < 0.00001);
                assert!((max - 56.0).abs() < 0.00001);
            }
            _ => panic!("unexpected schema!"),
        }

        assert_eq!(sensor.properties["detection"].epc, "0xB1");
        assert_eq!(sensor.properties["detection"].writable, false);
        assert_eq!(sensor.properties["detection"].observable, true);
        match &sensor.properties["detection"].schema {
            Schema::T(TypedSchema::Boolean { values }) => {
                assert_eq!(values.len(), 2);
                assert_eq!(values[0].value, true);
                assert_eq!(values[0].edt.as_ref().unwrap(), "0x41");
                assert_eq!(values[1].value, false);
                assert_eq!(values[1].edt.as_ref().unwrap(), "0x42");
            }
            _ => panic!("unexpected schema!"),
        }
    }

    #[test]
    fn read_illumination_sensor() {
        let sensor = read_def("tests/dd/illuminationSensor_vxxx.json").unwrap();
        assert_eq!(sensor.eoj, "0x000D");
        assert_eq!(sensor.properties["illumination_1"].epc, "0xE0");
        assert_eq!(sensor.properties["illumination_1"].writable, false);
        assert_eq!(sensor.properties["illumination_1"].observable, false);
        match &sensor.properties["illumination_1"].schema {
            Schema::T(TypedSchema::Number {
                unit: Some(u),
                minimum: Some(min),
                maximum: Some(max),
                multiple_of: None,
            }) => {
                assert_eq!(u, "Lux");
                assert!(min.abs() < 0.00001);
                assert!(max - 65533.0 < 0.00001);
            }
            _ => panic!("unexpected schema!"),
        }

        assert_eq!(sensor.properties["illumination_2"].epc, "0xE1");
        assert_eq!(sensor.properties["illumination_2"].writable, false);
        assert_eq!(sensor.properties["illumination_2"].observable, false);
        match &sensor.properties["illumination_2"].schema {
            Schema::T(TypedSchema::Number {
                unit: Some(u),
                minimum: Some(min),
                maximum: Some(max),
                multiple_of: None,
            }) => {
                assert_eq!(u, "KLux");
                assert!(min.abs() < 0.00001);
                assert!(max - 65533.0 < 0.00001);
            }
            _ => panic!("unexpected schema!"),
        }
    }

    #[test]
    #[ignore]
    fn read_device_descriptions_all_succeeds() {
        let path = "./tests/dd/";
        let dds = read_device_descriptions(path).expect("parsing dds failed!");
        assert!(dds.len() > 0);
        //currently we have... ten or so?
        assert_eq!(dds.len(), 10);
    }
}
