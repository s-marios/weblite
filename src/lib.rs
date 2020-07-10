use actix_web::{web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::prelude::*;
use std::io::{self, ErrorKind};

pub mod line_driver;
use line_driver::LineDriver;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub backend: String,
}

pub fn init_config() -> std::io::Result<Config> {
    let mut file = File::open("./config.json")?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let config: Config = serde_json::from_str(&contents)?;
    Ok(config)
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

pub async fn property(info: web::Path<(String, String)>) -> impl Responder {
    let response = format!("Property {}, of device: {}", info.1, info.0);
    HttpResponse::Ok().body(response)
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
) -> impl Responder {
    let connect = line_driver::LineDriver::new("150.65.230.118", 3361);
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
