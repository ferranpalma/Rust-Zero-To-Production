use once_cell::sync::Lazy;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use uuid::Uuid;

use zero2prod::{
    configuration::{get_configuration, DatabaseSettings},
    startup::Application,
    telemetry,
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

    let configuration = {
        let mut config = get_configuration().expect("Failed to read configuration");
        // Create a different db name for each test so every test is db isolated
        config.database.name = Uuid::new_v4().to_string();
        config.application.port = 0;

        config
    };

    let _ = configure_testing_database(&configuration.database).await;

    let application = Application::build_server(configuration.clone())
        .await
        .expect("Failed to build application server.");
    let web_address = format!("http://127.0.0.1:{}", application.get_application_port());

    let _ = tokio::spawn(application.run_application());

    TestingApp {
        web_address,
        db_pool: Application::get_db_connection_pool(&configuration.database),
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
