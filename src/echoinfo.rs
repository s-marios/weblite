use crate::descriptions::*;

pub struct EchonetDevice {
    pub host: String,
    pub eoj: String,
    pub properties: Vec<String>,
    pub description_id: usize,
}

impl EchonetDevice {
    pub fn has_property(&self, property: &str, descriptions: Descriptions) -> bool {
        descriptions
            .get(self.description_id)
            .expect("corrupt descriptions!")
            .properties
            .contains_key(property)
    }
}
