use actix_web::{web, HttpResponse, Responder};
use serde::Deserialize;

pub mod line_driver;
use line_driver::LineDriver;

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

#[derive(Deserialize, Debug)]
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

#[derive(Deserialize, Debug)]
pub struct EchoOperation {
    epc: String,
    edt: Option<Vec<String>>,
}

fn do_command(line: &mut LineDriver, command: &str) -> String {
    line.exec(command)
        .unwrap_or_else(|_| String::from("no data"))
}

fn generate_command(device: String, command: EchoCommand) -> Option<String> {
    if let EchoCommand::Request { esv, operations } = command {
        match esv.as_str() {
            //Get
            "0x62" => {
                let mut cmd = String::new();
                cmd.push_str(&device);
                cmd.push_str(&format!(":{}", operations[0].epc));
                cmd.push_str("\n");

                Some(cmd)
            }
            //SetGet
            "0x6E" => unimplemented!(),
            _ => None,
        }
    } else {
        None
    }
}

pub async fn echo_commands(
    device: web::Path<String>,
    command: web::Json<EchoCommand>,
) -> impl Responder {
    let connect = line_driver::LineDriver::new("150.65.230.118", 3361);
    match connect {
        Err(error) => String::from("failed to connect"),
        Ok(mut line) => {
            if let Some(cmd) = generate_command(device.into_inner(), command.into_inner()) {
                let result = do_command(&mut line, &cmd);
                print!("result: {}", result);
                result
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
