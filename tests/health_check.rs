use once_cell::sync::Lazy;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use std::net::TcpListener;
use uuid::Uuid;

use zero2prod::{
    configuration::{get_configuration, DatabaseSettings},
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

async fn spawn_app() -> TestingApp {
    Lazy::force(&TRACING);

    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind random port");
    let port = listener.local_addr().unwrap().port();
    let web_address = format!("http://127.0.0.1:{}", port);

    let mut config = get_configuration().expect("Failed to read configuration");
    // Create a different db name for each test so every test is db isolated
    config.database.name = Uuid::new_v4().to_string();
    let db_pool = configure_testing_database(&config.database).await;

    let server = startup::run(listener, db_pool.clone()).expect("Failed to bind address");

    let _ = tokio::spawn(server);

    TestingApp {
        web_address,
        db_pool,
    }
}

async fn configure_testing_database(config: &DatabaseSettings) -> PgPool {
    // Connect to postgres, not to a specific postgres database
    let mut db_connection = PgConnection::connect(&config.get_connection_string_without_db())
        .await
        .expect("Failed to connect to Postgres");

    // Create and migrate the database
    db_connection
        .execute(format!(r#"CREATE DATABASE "{}";"#, config.name).as_str())
        .await
        .expect("Failed to create database");

    let db_pool = PgPool::connect(&config.get_connection_string())
        .await
        .expect("Failed to connect to Postgres");

    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .expect("Failed to migrate database");

    db_pool
}

#[actix_web::test]
async fn test_health_check_works() {
    // The health check function should return a 200 OK with empty body

    let app = spawn_app().await;

    let client = reqwest::Client::new();

    let response = client
        .get(format!("{}/health_check", &app.web_address))
        .send()
        .await
        .expect("Failed to execute the request");

    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}

#[actix_web::test]
async fn test_susbcribe_works_with_valid_data() {
    let app = spawn_app().await;
    let client = reqwest::Client::new();

    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";
    let response = client
        .post(&format!("{}/subscriptions", &app.web_address))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status().as_u16(), 200);

    // Assert that the actual db operation has been done
    // sqlx::query!() defines a db_data struct at compile time with one field per column
    let db_data = sqlx::query!("SELECT email, name FROM subscriptions",)
        .fetch_one(&app.db_pool)
        .await
        .expect("Failed to fetch subscriptions");

    assert_eq!(db_data.email, "ursula_le_guin@gmail.com");
    assert_eq!(db_data.name, "le guin");
}

#[actix_web::test]
async fn test_subscribe_fails_with_invalid_data() {
    let app = spawn_app().await;
    let client = reqwest::Client::new();
    let cases = vec![
        ("name=le%guin", "missing email"),
        ("email=ursula_le_guin%40gmail.com", "missing name"),
        ("", "missin name and email"),
    ];

    for (case, error) in cases {
        let response = client
            .post(&format!("{}/subscriptions", &app.web_address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(case)
            .send()
            .await
            .expect("Failed to execute request");

        assert_eq!(
            response.status().as_u16(),
            400,
            "The API did not fail when the payload error was: {}",
            error
        );
    }
}
