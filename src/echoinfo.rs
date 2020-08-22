use crate::descriptions::*;

#[derive(Debug)]
pub(crate) struct DeviceInfo {
    pub host: String,
    pub eoj: String,
    pub r_prop: Vec<String>,
    pub w_prop: Vec<String>,
}

impl DeviceInfo {
    pub fn new(host: String, eoj: String) -> Self {
        DeviceInfo {
            host,
            eoj,
            r_prop: Vec::new(),
            w_prop: Vec::new(),
        }
    }
}

//#[derive(Debug)]
//pub(crate) struct EchonetDevice {
//    pub host: String,
//    pub eoj: String,
//    //pub r_conv:
//}
