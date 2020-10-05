use crate::echoinfo::{DeviceInfo, DeviceProtocolInfo};
use crate::hex;
use crate::line_driver::LineDriver;
use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;

#[derive(PartialEq, Eq, Debug)]
pub enum LineResult {
    OK,
    NG,
}

#[derive(Debug)]
pub(crate) struct LineResponse<'a> {
    pub result: LineResult,
    pub host: &'a str,
    pub eoj: &'a str,
    pub property: &'a str,
    pub data: Option<&'a str>,
}

impl LineResponse<'_> {
    pub fn hosteoj(&self) -> String {
        format!("{}:0x{}", self.host, self.eoj)
    }

    pub fn hexclass(&self) -> String {
        format!("0x{}", &self.eoj[..4])
    }

    pub fn eoj(&self) -> String {
        format!("0x{}", &self.eoj)
    }

    pub fn host(&self) -> String {
        self.host.to_string()
    }

    pub fn is_ok(&self) -> bool {
        match self.result {
            LineResult::OK => true,
            LineResult::NG => false,
        }
    }
}

impl<'a> TryFrom<&'a str> for LineResponse<'a> {
    type Error = ();
    fn try_from(source: &'a str) -> std::result::Result<Self, ()> {
        let source = source.trim();
        let parts = source
            .split(|c| c == ':' || c == ',')
            .collect::<Vec<&str>>();

        let result = match *parts.get(0).ok_or(())? {
            "OK" => LineResult::OK,
            "NG" => LineResult::NG,
            _ => return Err(()),
        };
        let host = parts.get(1).ok_or(())?;
        let eoj = parts.get(2).ok_or(())?;
        let property = parts.get(3).ok_or(())?;
        let data = parts.get(4).copied();
        Ok(LineResponse {
            result,
            host,
            eoj,
            property,
            data,
        })
    }
}

const GETALLNODES: &str = "224.0.23.0:0ef000:0xD7";
pub(super) fn get_all_classes(driver: &mut LineDriver) -> std::io::Result<HashSet<String>> {
    let res = driver.exec_multi(GETALLNODES)?;

    Ok(res
        .iter()
        .map(|r| LineResponse::try_from(r.as_ref()))
        //make sure we have a valid entry
        .filter_map(|opt_r| opt_r.ok())
        .inspect(|lr| println!("line response: {:?}", lr))
        //keep only the data
        .filter_map(|r| r.data)
        //chop 4 bytes
        .map(|d| &d[4..])
        //keep all non-empty stuff
        .filter(|d| !d.is_empty())
        .map(|d| d.chars())
        .flatten()
        .collect::<Vec<_>>()
        .chunks(4)
        .map(|chunk| chunk.iter().collect::<String>())
        .collect::<HashSet<_>>())
}

pub(super) fn class_intersect(
    available: &HashSet<String>,
    discovered: &HashSet<String>,
    //eojs have "0x" prefix in the response
) -> HashSet<String> {
    //do we cover all the discovered eojs?
    let diff_string = discovered
        .difference(available)
        .cloned()
        .collect::<Vec<String>>()
        .join(", ");
    if !diff_string.is_empty() {
        println!("Warning: the following device classes were detected but *not* covered by the device definitions:");
        println!("{}", diff_string);
    } else {
        println!("All discovered classes are covered");
    }

    let intersection = available.intersection(discovered);

    //we need to map to "0x0000" format
    intersection
        .map(|class| format!("0x{}", class))
        .collect::<HashSet<String>>()
}

pub(super) fn scan_classes(
    classes: HashSet<String>,
    driver: &mut LineDriver,
    //eojs have "0x" prefix in the response
) -> HashMap<String, DeviceInfo> {
    let cmd = classes
        .into_iter()
        //add the "00" instance to them
        //and generate the get property map command
        //two commands
        .map(|eoj| format!("224.0.23.0:{}00:0x9F\n224.0.23.0:{0}00:0x9E\n", eoj))
        .collect::<String>();
    println!("Generated Scan command:\n{}", cmd);

    //for the unwrap, see TODO in line_driver
    let res = driver.exec_multi(&cmd).unwrap();
    println!("scan results:\n{:?}", res);

    let parsed = res
        .iter()
        .map(|r| LineResponse::try_from(r as &str).unwrap())
        .collect::<Vec<LineResponse>>();

    generate_devices(parsed)
}

//eojs have "0x" prefix in the response
fn generate_devices(parsed: Vec<LineResponse>) -> HashMap<String, DeviceInfo> {
    let mut set = HashMap::with_capacity(parsed.len());
    for resp in parsed {
        //check for valid data
        if resp.result == LineResult::NG || resp.data == None {
            continue;
        }

        //we're good to go
        let entry = set
            .entry(resp.hosteoj())
            .or_insert_with(|| DeviceInfo::new(resp.host(), resp.eoj()));

        let props_u8 = hex::to_bytes(resp.data.unwrap());
        if props_u8 == None {
            println!("hex to bytes conversion failed!");
            continue;
        }

        //safe to unwrap because of the above
        if let Some(props) = parse_property_map(&props_u8.unwrap()) {
            let text_props = props
                .into_iter()
                .map(|prop| format!("0x{:2X}", prop))
                .collect::<Vec<String>>();
            match resp.property {
                "0x9F" => entry.r.extend(text_props.into_iter()),
                "0x9E" => entry.w.extend(text_props.into_iter()),
                other => println!("generate_devices: suspicious property: {}", other),
            }
        }
    }

    set
}

pub fn parse_property_map(property_map: &[u8]) -> Option<Vec<u8>> {
    let howmany = *property_map.get(0)?;
    if howmany < 16u8 {
        //we trust that whatever we were sent is .. correct..
        //we don't check if declared length actually matches
        Some(property_map[1..].to_vec())
    } else {
        parse_binary_map(property_map)
    }
}

fn parse_binary_map(property_map: &[u8]) -> Option<Vec<u8>> {
    if property_map.len() != 17 {
        return None;
    }
    Some(
        property_map[1..]
            .iter()
            .enumerate()
            .fold(vec![], |mut acc, (nth, byte)| {
                (0..8).for_each(|i| {
                    if byte & (1u8 << i) != 0 {
                        acc.push(0x80u8 + (nth as u8) + (0x10u8 * i))
                    }
                });
                acc
            }),
    )
}

pub(super) fn scan_protoinfo(
    classes: HashSet<String>,
    driver: &mut LineDriver,
    //eojs have "0x" prefix in the response
) -> std::io::Result<Vec<DeviceProtocolInfo>> {
    fn map_info(info: DeviceProtocolInfo, responses: &[LineResponse]) -> DeviceProtocolInfo {
        //get protocol info
        let info = if let Some(proto_resp) = responses.iter().find(|resp| {
            info.id.starts_with(resp.host)
                && resp.is_ok()
                && resp.property == "0x82"
                && resp.data.is_some()
        }) {
            info.with_protocol(proto_resp.data.unwrap().to_string())
        } else {
            info
        };

        //get manufacturer code
        let info = if let Some(man_resp) = responses.iter().find(|resp| {
            info.id.starts_with(resp.host)
                && resp.is_ok()
                && resp.property == "0x8A"
                && resp.data.is_some()
        }) {
            info.with_code(man_resp.data.unwrap().to_string())
        } else {
            info
        };

        info
    }

    let protoinfocommand = "224.0.23.0:0ef000:0x82\n".to_string();
    let manufacturercommand = "224.0.23.0:0ef000:0x8A\n".to_string();
    let appendixinfocommand = classes
        .iter()
        .map(|class| format!("224.0.23.0:{}00:0x82\n", class))
        .collect::<String>();
    let command = std::iter::once(protoinfocommand)
        .chain(std::iter::once(manufacturercommand))
        .chain(std::iter::once(appendixinfocommand))
        .collect::<String>();

    println!("scan_protoinfo command:\n{}", command);
    let res = driver.exec_multi(&command)?;
    let responses = res
        .iter()
        .map(|response| {
            LineResponse::try_from(response.as_ref()).expect("died when collecting information")
        })
        .collect::<Vec<LineResponse<'_>>>();
    //now, use the appendix info to get stuff done first
    let infos = responses
        .iter()
        .filter(|lr| lr.property == "0x82" && !lr.eoj.starts_with("0ef0"))
        .map(|lr| (lr, DeviceProtocolInfo::new(lr.hosteoj())))
        .map(|(lr, info)| {
            if let Some(data) = lr.data {
                info.with_appendix(data.to_string())
            } else {
                info
            }
        })
        //TODO process the rest of the stuff
        .collect::<Vec<_>>();
    let infos = infos
        .into_iter()
        .map(|info| {
            //for each piece we have, update it by looking into the responses
            map_info(info, &responses)
        })
        .collect::<Vec<_>>();
    Ok(infos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_property_map_is_none() {
        assert_eq!(parse_property_map(&[]).is_none(), true);
    }

    #[test]
    fn zero_properties_is_ok() {
        let res = parse_property_map(&[0]).unwrap();
        assert!(res.is_empty());
    }

    #[test]
    fn property_maps_simple_ok() {
        let pmap = [3u8, 0x80u8, 0x9Eu8, 0x9Du8];
        let expected = [0x80u8, 0x9Eu8, 0x9D];
        assert_eq!(parse_property_map(&pmap).unwrap(), expected);
    }

    #[test]
    fn property_maps_binary_check() {
        let bmap = [
            24, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0xFF,
        ];
        let result = parse_property_map(&bmap).unwrap();
        let expected = vec![
            0x80u8, 0x90u8, 0xA0u8, 0xB0u8, 0xC0u8, 0xD0u8, 0xE0u8, 0xF0u8, 0x81u8, 0x91u8, 0xA1u8,
            0xB1u8, 0xC1u8, 0xD1u8, 0xE1u8, 0xF1u8, 0x8Fu8, 0x9Fu8, 0xAFu8, 0xBFu8, 0xCFu8, 0xDFu8,
            0xEFu8, 0xFFu8,
        ];
        println!("result: {:?}", result);
        assert_eq!(result.len(), 24);
        assert_eq!(result, expected);
    }
}
