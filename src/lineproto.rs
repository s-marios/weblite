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
