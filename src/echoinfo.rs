use crate::converters::Converter;
use crate::descriptions::TextDescription;
use serde::Serialize;
#[derive(Debug)]
pub(crate) struct DeviceInfo {
    pub host: String,
    pub eoj: String,
    //these are epcs
    pub r: Vec<String>,
    pub w: Vec<String>,
}

impl DeviceInfo {
    pub fn new(host: String, eoj: String) -> Self {
        DeviceInfo {
            host,
            eoj,
            r: Vec::new(),
            w: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct EchonetDevice {
    pub host: String,
    pub eoj: String,
    pub properties: Vec<EchonetProperty>,
}

impl EchonetDevice {
    pub fn new(host: String, eoj: String) -> Self {
        EchonetDevice {
            host,
            eoj,
            properties: vec![],
        }
    }

    pub fn add(&mut self, property: EchonetProperty) {
        self.properties.push(property)
    }

    pub(crate) fn combine(info: DeviceInfo, properties: Vec<EchonetProperty>) -> EchonetDevice {
        let properties = properties
            .into_iter()
            .filter(|prop| info.r.contains(&prop.epc) || info.w.contains(&prop.epc))
            .map(|mut prop| {
                prop.w = info.w.contains(&prop.epc);
                prop
            })
            .collect();

        EchonetDevice {
            properties,
            host: info.host,
            eoj: info.eoj,
        }
    }

    pub fn hosteoj(&self) -> String {
        format!("{}:{}", self.host, self.eoj)
    }
}

#[derive(Clone, Debug)]
pub struct EchonetProperty {
    pub converter: Converter,
    pub w: bool,
    pub epc: String,
    pub name: String,
}

impl EchonetProperty {
    pub fn new(name: String, epc: String, w: bool, converter: Converter) -> EchonetProperty {
        EchonetProperty {
            name,
            epc,
            w,
            converter,
        }
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct DeviceProtocolInfo {
    pub id: String,
    pub device_type: String,
    pub protocol: ProtocolInfo,
    pub manufacturer: ManufacturerInfo,
}

impl DeviceProtocolInfo {
    pub fn new(id: String) -> DeviceProtocolInfo {
        DeviceProtocolInfo {
            id,
            device_type: String::from("unavailable"),
            protocol: ProtocolInfo {
                protocol: String::from("N/A"),
                appendix: String::from("N/A"),
            },
            manufacturer: ManufacturerInfo {
                code: String::from("N/A"),
                descriptions: TextDescription {
                    en: String::from("N/A"),
                    ja: String::from("不明"),
                },
            },
        }
    }

    pub fn with_type(self, device_type: String) -> Self {
        DeviceProtocolInfo {
            device_type,
            ..self
        }
    }

    pub fn with_protocol(self, protocol: String) -> Self {
        DeviceProtocolInfo {
            protocol: ProtocolInfo {
                protocol,
                ..self.protocol
            },
            ..self
        }
    }

    pub fn with_appendix(self, appendix: String) -> Self {
        DeviceProtocolInfo {
            protocol: ProtocolInfo {
                appendix,
                ..self.protocol
            },
            ..self
        }
    }

    pub fn with_code(self, code: String) -> Self {
        DeviceProtocolInfo {
            manufacturer: ManufacturerInfo {
                code,
                ..self.manufacturer
            },
            ..self
        }
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct ProtocolInfo {
    #[serde(rename = "type")]
    pub protocol: String,
    #[serde(rename = "version")]
    pub appendix: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct ManufacturerInfo {
    pub code: String,
    pub descriptions: TextDescription,
}
