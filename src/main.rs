use sqlx::PgPool;
use std::net::TcpListener;

use zero2prod::{configuration, startup, telemetry};

#[actix_web::main]
async fn main() -> Result<(), std::io::Error> {
    let configuration = configuration::get_configuration().expect("Failed to read configuration");

    telemetry::Telemetry::create("zero2prod".into(), "info".into(), std::io::stdout);

    let db_connection_pool = PgPool::connect(&configuration.database.get_connection_string())
        .await
        .expect("Failed to connect to Postgres");

    let address = format!("127.0.0.1:{}", configuration.application_port);
    let listener = TcpListener::bind(address)?;
    startup::run(listener, db_connection_pool)?.await
}
