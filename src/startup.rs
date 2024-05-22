use actix_web::{dev::Server, web, App, HttpServer};
use sqlx::PgPool;
use std::net::TcpListener;
use tracing_actix_web::TracingLogger;

use crate::{
    email_client::EmailClient,
    routes::{health_check, subscriptions},
};

pub fn run(
    listener: TcpListener,
    db_pool: PgPool,
    email_client: EmailClient,
) -> Result<Server, std::io::Error> {
    // Wrap the pool in web::Data, that ends up as an Arc pointer
    let db_connection = web::Data::new(db_pool);
    let email_client = web::Data::new(email_client);
    let server = HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger::default())
            .route("/health_check", web::get().to(health_check::health_check))
            .route("/subscriptions", web::post().to(subscriptions::subscribe))
            .app_data(db_connection.clone())
            .app_data(email_client.clone())
    })
    .listen(listener)?
    .run();

    Ok(server)
}
