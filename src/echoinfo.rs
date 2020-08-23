use crate::converters::Converter;
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
