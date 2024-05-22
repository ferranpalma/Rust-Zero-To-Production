use once_cell::sync::Lazy;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use std::net::TcpListener;
use uuid::Uuid;

use zero2prod::{
    configuration::{get_configuration, DatabaseSettings},
    email_client::EmailClient,
    startup, telemetry,
};

static TRACING: Lazy<()> = Lazy::new(|| {
    if std::env::var("TEST_LOG").is_ok() {
        telemetry::Telemetry::create("test".into(), "debug".into(), std::io::stdout);
    } else {
        telemetry::Telemetry::create("test".into(), "debug".into(), std::io::sink);
    }
});

pub struct TestingApp {
    pub web_address: String,
    pub db_pool: PgPool,
}

pub async fn spawn_app() -> TestingApp {
    Lazy::force(&TRACING);

    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind random port");
    let port = listener.local_addr().unwrap().port();
    let web_address = format!("http://127.0.0.1:{}", port);

    let mut configuration = get_configuration().expect("Failed to read configuration");
    // Create a different db name for each test so every test is db isolated
    configuration.database.name = Uuid::new_v4().to_string();
    let db_pool = configure_testing_database(&configuration.database).await;

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

    let server =
        startup::run(listener, db_pool.clone(), email_client).expect("Failed to bind address");

    let _ = tokio::spawn(server);

    TestingApp {
        web_address,
        db_pool,
    }
}

async fn configure_testing_database(config: &DatabaseSettings) -> PgPool {
    // Connect to postgres, not to a specific postgres database
    let mut db_connection = PgConnection::connect_with(&config.connect_database_engine())
        .await
        .expect("Failed to connect to Postgres");

    // Create and migrate the database
    db_connection
        .execute(format!(r#"CREATE DATABASE "{}";"#, config.name).as_str())
        .await
        .expect("Failed to create database");

    let db_pool = PgPool::connect_with(config.connect_database_instance())
        .await
        .expect("Failed to connect to Postgres");

    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .expect("Failed to migrate database");

    db_pool
}
