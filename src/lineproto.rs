use crate::line_driver::LineDriver;
use std::collections::HashSet;
use std::convert::TryFrom;

#[derive(Debug)]
enum LineResult {
    OK,
    NG,
}

#[derive(Debug)]
struct LineResponse<'a> {
    result: LineResult,
    host: &'a str,
    eoj: &'a str,
    property: &'a str,
    data: Option<&'a str>,
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
    println!("classes to search for: {:?}", intersection);

    intersection.cloned().collect::<HashSet<String>>()
}

pub(super) fn scan_classes(classes: HashSet<String>, driver: &mut LineDriver) -> Vec<String> {
    let cmd = classes
        .into_iter()
        //add the "00" instance to them
        //and generate the get property map command
        .map(|eoj| format!("224.0.23.0:{}00:0x9F\n", eoj))
        .collect::<String>();
    println!("Generated Scan command:\n{}", cmd);
    let res = driver.exec_multi(&cmd);
    println!("scan results:\n{:?}", res);
    unimplemented!()
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
