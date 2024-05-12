use sqlx::{Connection, PgConnection};
use std::net::TcpListener;

use zero2prod::{configuration, startup};

fn spawn_app() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind random port");
    let port = listener.local_addr().unwrap().port();

    let server = startup::run(listener).expect("Failed to bind address");

    let _ = tokio::spawn(server);

    format!("http://127.0.0.1:{}", port)
}

#[actix_web::test]
async fn test_health_check_works() {
    // The health check function should return a 200 OK with empty body

    let address = spawn_app();

    let client = reqwest::Client::new();

    let response = client
        .get(format!("{}/health_check", &address))
        .send()
        .await
        .expect("Failed to execute the request");

    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}

#[actix_web::test]
async fn test_susbcribe_works_with_valid_data() {
    let address = spawn_app();
    let client = reqwest::Client::new();

    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";
    let response = client
        .post(&format!("{}/subscriptions", &address))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status().as_u16(), 200);

    // Assert that the actual db operation has been done
    let config = configuration::get_configuration().expect("Failed to read configuration");
    let connection_string = config.database.get_connection_string();

    let mut connection = PgConnection::connect(&connection_string)
        .await
        .expect("Failed to connect to Postgres");

    // sqlx::query!() defines a db_data struct at compile time with one field per column
    let db_data = sqlx::query!("SELECT email, name FROM subscriptions",)
        .fetch_one(&mut connection)
        .await
        .expect("Failed to fetch subscriptions");

    assert_eq!(db_data.email, "ursula_le_guin@gmail.com");
    assert_eq!(db_data.name, "le guin");
}

#[actix_web::test]
async fn test_subscribe_fails_with_invalid_data() {
    let address = spawn_app();
    let client = reqwest::Client::new();
    let cases = vec![
        ("name=le%guin", "missing email"),
        ("email=ursula_le_guin%40gmail.com", "missing name"),
        ("", "missin name and email"),
    ];

    for (case, error) in cases {
        let response = client
            .post(&format!("{}/subscriptions", &address))
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
