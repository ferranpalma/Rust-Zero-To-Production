use secrecy::ExposeSecret;
use sqlx::PgPool;
use std::net::TcpListener;

use zero2prod::{configuration, startup, telemetry};

#[actix_web::main]
async fn main() -> Result<(), std::io::Error> {
    let configuration = configuration::get_configuration().expect("Failed to read configuration");

    telemetry::Telemetry::create("zero2prod".into(), "info".into(), std::io::stdout);

    let db_connection_pool = PgPool::connect_lazy(
        configuration
            .database
            .get_connection_string()
            .expose_secret(),
    )
    .expect("Failed to connect to Postgres");

    let address = format!(
        "{}:{}",
        configuration.application.address, configuration.application.port
    );
    let listener = TcpListener::bind(address)?;
    startup::run(listener, db_connection_pool)?.await
}
