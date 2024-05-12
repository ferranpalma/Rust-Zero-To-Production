use actix_web::{dev::Server, web, App, HttpServer};
use std::net::TcpListener;

use crate::routes::{health_check, subscriptions};

pub fn run(listener: TcpListener) -> Result<Server, std::io::Error> {
    let server = HttpServer::new(|| {
        App::new()
            .route("/health_check", web::get().to(health_check::health_check))
            .route("/subscriptions", web::post().to(subscriptions::subscribe))
    })
    .listen(listener)?
    .run();

    Ok(server)
}
