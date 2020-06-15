use actix_web::{web, HttpResponse, Responder};

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

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
