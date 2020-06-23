use actix_web::{web, HttpResponse, Responder};
use serde::Deserialize;

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
pub struct EchoCommand {
    request: EchoRequest,
}

#[derive(Deserialize, Debug)]
pub struct EchoRequest {
    esv: String,
    operations: Vec<EchoOperation>,
}

#[derive(Deserialize, Debug)]
pub struct EchoOperation {
    epc: String,
    edt: Option<Vec<String>>,
}

pub async fn echo_commands(
    device: web::Path<String>,
    command: web::Json<EchoCommand>,
) -> impl Responder {
    let response = format!(
        "echo commands for device id: {}\nJSON (raw): {:?}",
        device, command
    );
    HttpResponse::Ok().body(response)
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
