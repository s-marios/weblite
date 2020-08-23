use actix_web::{web, App, HttpServer};
use weblite::*;

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    let data = web::Data::new(init()?);
    println!("backend: {}", data.config.backend);

    HttpServer::new(move || {
        App::new().service(
            web::scope("/elapi")
                .app_data(data.clone())
                .route("", web::get().to(versions))
                .route("/v1", web::get().to(resources))
                .route("/v1/controllers", web::get().to(controllers))
                .route("/v1/devices", web::get().to(devices))
                .route("/v1/devices/{deviceid}", web::get().to(device))
                .route(
                    "/v1/devices/{deviceid}/properties",
                    web::get().to(properties),
                )
                .route(
                    "/v1/devices/{deviceid}/properties/{propertyid}",
                    web::put().to(set_property),
                )
                .route(
                    "/v1/devices/{deviceid}/properties/{propertyid}",
                    web::get().to(get_property),
                )
                .route(
                    "/v1/devices/{deviceid}/echoCommands",
                    web::post().to(echo_commands),
                ),
        )
    })
    .bind("127.0.0.1:8088")?
    .run()
    .await
}
