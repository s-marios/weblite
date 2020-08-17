use serde::Deserialize;

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
pub struct PropertyValue<T> {
    pub value: T,
    pub edt: String,
}

#[derive(Deserialize, Debug)]
pub struct PropertyInfo {
    pub code: String,
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
        multipleOf: Option<f32>,
    },
    Null {
        edt: String,
    },

    Object {
        order: Vec<PropertyInfo>,
    },
    Array,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptions;

    #[test]
    fn ai_api() {
        println!("test!!!");
        let ais: Entries = descriptions::read_def_generic("./tests/ai/test.json").unwrap();
        assert_eq!(ais.entries[0].eoj, "0290");
        assert_eq!(ais.entries[1].ai.is_empty(), true);
    }
}
