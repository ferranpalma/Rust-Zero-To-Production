use sqlx::PgPool;
use std::net::TcpListener;

use zero2prod::{configuration, email_client::EmailClient, startup, telemetry};

#[actix_web::main]
async fn main() -> Result<(), std::io::Error> {
    let configuration = configuration::get_configuration().expect("Failed to read configuration");

    telemetry::Telemetry::create("zero2prod".into(), "info".into(), std::io::stdout);

    let db_connection_pool =
        PgPool::connect_lazy_with(configuration.database.connect_database_instance());

    // Build a SubscriberEmail instance
    let sender_email = configuration
        .email_client
        .sender()
        .expect("Invalid sender email address.");
    let http_client_timeout = configuration.email_client.timeout();
    let email_client = EmailClient::new(
        configuration.email_client.base_url,
        sender_email,
        configuration.email_client.authorization_token,
        http_client_timeout,
    );

    let address = format!(
        "{}:{}",
        configuration.application.address, configuration.application.port
    );
    let listener = TcpListener::bind(address)?;
    startup::run(listener, db_connection_pool, email_client)?.await
}
