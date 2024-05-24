use wiremock::{
    matchers::{method, path},
    Mock, ResponseTemplate,
};

use crate::helpers::spawn_app;

#[actix_web::test]
async fn test_susbcribe_works_with_valid_data() {
    let app = spawn_app().await;

    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";
    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    let response = app.send_subscription_request(body.into()).await;
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
    let cases = vec![
        ("name=le%guin", "missing email"),
        ("email=ursula_le_guin%40gmail.com", "missing name"),
        ("name=&email=", "empty name and email"),
        ("name=&email=ursula_le_guin%40gmail.com", "empty name"),
        ("name=Ursula&email=", "empty email"),
        ("name=Ursula&email=definitely-not-an-email", "invalid email"),
    ];

    for (case, error) in cases {
        let response = app.send_subscription_request(case.into()).await;
        assert_eq!(
            response.status().as_u16(),
            400,
            "The API did not fail when the payload error was: {}",
            error
        );
    }
}

#[actix_web::test]
async fn test_subscribe_sends_confirmation_email_for_valid_data() {
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    app.send_subscription_request(body.into()).await;

    // Mock asserts on drop
}
