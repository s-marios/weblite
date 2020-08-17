use actix_web::{web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::{self, ErrorKind};
use std::path::Path;

mod converters;
pub mod descriptions;
pub mod echoinfo;
pub mod hex;
pub mod line_driver;
mod lineproto;

pub use descriptions::*;
pub use hex::*;
use line_driver::LineDriver;

#[derive(Debug)]
pub struct AppData {
    pub config: Config,
    pub descriptions: Descriptions,
}

#[derive(Deserialize, Debug)]
pub struct Config {
    pub backend: String,
    pub dd_dir: String,
}

pub fn init() -> std::io::Result<AppData> {
    let config = init_config()?;
    let mut driver = LineDriver::from(&config.backend)?;
    let descriptions = read_device_descriptions(&config.dd_dir)?;
    println!("available device descriptions:");
    descriptions
        .iter()
        .for_each(|dd| println!("type (class): {} ({})", dd.device_type, dd.eoj));

    let discovered = lineproto::get_all_classes(&mut driver)?;
    println!("discovered classes: {:?}", discovered);

    //get all available device description classes
    let available = descriptions
        .iter()
        .map(|dd| dd.eoj[2..].to_string())
        .collect::<HashSet<String>>();

    let intersection = lineproto::class_intersect(&available, &discovered);
    lineproto::scan_classes(intersection, &mut driver);

    Ok(AppData {
        config,
        descriptions,
    })
}

pub fn init_config() -> std::io::Result<Config> {
    let contents = std::fs::read_to_string("./config.json")?;
    let config: Config = serde_json::from_str(&contents)?;
    Ok(config)
}

pub fn init_device_descriptions<P: AsRef<Path>>(dir: P) -> std::io::Result<Descriptions> {
    read_device_descriptions(dir)
}

pub async fn versions() -> impl Responder {
    HttpResponse::Ok().body("TODO: elapi versions here")
}
pub async fn resources() -> impl Responder {
    HttpResponse::Ok().body("TODO: resources here")
}
pub async fn devices() -> impl Responder {
    HttpResponse::Ok().body("TODO: devices here")
}

pub async fn controllers() -> impl Responder {
    HttpResponse::Ok().body("TODO: controllers here")
}

pub async fn device(info: web::Path<String>) -> impl Responder {
    let response = format!("Details for device: {}", info);
    HttpResponse::Ok().body(response)
}

pub async fn properties(info: web::Path<String>) -> impl Responder {
    let response = format!("Properties of device: {}", info);
    HttpResponse::Ok().body(response)
}

pub async fn property(
    info: web::Path<(String, String)>,
    data: web::Data<AppData>,
) -> impl Responder {
    let response = format!("Property {}, of device: {}", info.1, info.0);
    HttpResponse::Ok().body(response)
}

pub async fn scan(config: web::Data<Config>) -> impl Responder {
    let response = String::from("scan results:");
    let line = LineDriver::from(&config.backend);
    if let Err(err) = line {
        return HttpResponse::Ok().body(response);
    }
    let mut line = line.unwrap();
    let res = line.exec_multi("224.0.23.0:0ef000:d6");
    match res {
        Err(error) => unimplemented!(),
        Ok(lines) => HttpResponse::Ok().body(format!("resp: {}", lines.join(""))),
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum EchoCommand {
    #[serde(rename = "request")]
    Request {
        esv: String,
        operations: Vec<EchoOperation>,
    },
    #[serde(rename = "response")]
    Response {
        esv: String,
        operations: Vec<EchoOperation>,
    },
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EchoOperation {
    epc: String,
    edt: Option<Vec<String>>,
}

fn do_command(line: &mut LineDriver, command: &str) -> io::Result<String> {
    line.exec(command)
}

fn generate_commands(
    device: String,
    service: &mut String,
    command: EchoCommand,
) -> Option<Vec<String>> {
    if let EchoCommand::Request { esv, operations } = command {
        *service = esv.clone();
        match esv.as_str() {
            //Get
            "0x62" => Some(operations.iter().fold(vec![], |mut v, op| {
                let mut cmd = String::new();
                cmd.push_str(&device);
                cmd.push_str(&format!(":{}", op.epc));
                cmd.push_str("\n");
                v.push(cmd);
                v
            })),
            //SetGet
            "0x6E" => unimplemented!(),
            _ => None,
        }
    } else {
        None
    }
}

fn response_to_echo_result(
    mut _service: String,
    responses: Vec<String>,
) -> Result<EchoCommand, io::Error> {
    //gather all the pieces of information...

    let mut esv = "0x72".to_string();
    let ops = responses
        .iter()
        .map(|response| {
            let mut tokens = response.split(',');
            let okng = tokens.next()?;
            //TODO: why this doesn't work with
            //starts_with, == etc? and only works with contains?!?
            if okng.contains("NG") {
                esv = "0x52".to_string();
            }

            let target = tokens.next()?;
            let epc = target.rsplit(':').next()?.to_string();
            let opt_data = tokens.next();

            let edt: Option<Vec<String>> = if let Some(data) = opt_data {
                Some(
                    data.chars()
                        .skip(2)
                        .collect::<Vec<char>>()
                        .chunks_exact(2)
                        .map(|chunk| chunk.iter().collect::<String>())
                        .map(|hex| String::from("0x") + &hex)
                        .collect::<Vec<String>>(),
                )
            } else {
                None
            };
            Some(EchoOperation { epc, edt })
        })
        .collect::<Option<Vec<EchoOperation>>>();

    match ops {
        None => Err(io::Error::new(
            ErrorKind::InvalidInput,
            "an op was malformed!",
        )),
        Some(operations) => Ok(EchoCommand::Response { esv, operations }),
    }
}

pub async fn echo_commands(
    device: web::Path<String>,
    command: web::Json<EchoCommand>,
    data: web::Data<AppData>,
) -> impl Responder {
    let connect = line_driver::LineDriver::from(&data.config.backend);
    match connect {
        Err(error) => format!("failed to connect: {}", error),
        Ok(mut line) => {
            let command = command.into_inner();
            let device = device.into_inner();

            let mut service = String::new();
            if let Some(commands) = generate_commands(device, &mut service, command) {
                let mut keep_going = true;
                let results = commands
                    .iter()
                    .map(|cmd| do_command(&mut line, cmd))
                    .try_fold(vec![], |mut acc, cmd_res| {
                        if !keep_going {
                            return Err("stop");
                        }
                        match cmd_res {
                            Ok(res) => {
                                keep_going = res.starts_with("OK");
                                acc.push(res);
                                Ok(acc)
                            }
                            _ => Err("transmition error"),
                        }
                    });

                match results {
                    Ok(strings) => serde_json::to_string_pretty(
                        &response_to_echo_result(service, strings).unwrap(),
                    )
                    .unwrap(),
                    Err(error) => error.to_string(),
                }
            } else {
                String::from("bad command")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
