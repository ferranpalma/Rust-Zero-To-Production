use wiremock::{
    matchers::{method, path},
    Mock, ResponseTemplate,
};

use crate::helpers::spawn_app;

#[actix_web::test]
async fn test_requests_without_token_are_rejected() {
    let app = spawn_app().await;

    let response = reqwest::get(&format!("{}/subscriptions/confirm", app.web_address))
        .await
        .unwrap();

    assert_eq!(response.status().as_u16(), 400);
}

#[actix_web::test]
async fn test_request_with_unauthorized_token_fails() {
    let app = spawn_app().await;

    let unauthorized_token = "cdef146lnj09inI890nhBKLk0";

    let response = reqwest::get(&format!(
        "{}/subscriptions/confirm?subscription_token={}",
        app.web_address, unauthorized_token
    ))
    .await
    .unwrap();

    assert_eq!(response.status().as_u16(), 401);
}

#[actix_web::test]
async fn test_confirmation_link_works() {
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    app.send_subscription_request(body.into()).await;

    let email_client_response = &app.email_server.received_requests().await.unwrap()[0];
    let confirmation_link = app.get_confirmation_link(email_client_response);

    // Using text_link or html_link does not matter. Moreover, there is a test that checks that
    // both are the same
    let response = reqwest::get(confirmation_link.text_link).await.unwrap();
    assert_eq!(response.status().as_u16(), 200);
}

#[actix_web::test]
async fn test_confirmation_link_changes_database_status() {
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    app.send_subscription_request(body.into()).await;

    let email_client_response = &app.email_server.received_requests().await.unwrap()[0];
    let confirmation_link = app.get_confirmation_link(email_client_response);

    // Using text_link or html_link does not matter. Moreover, there is a test that checks that
    // both are the same
    let response = reqwest::get(confirmation_link.text_link).await.unwrap();
    assert_eq!(response.status().as_u16(), 200);

    let db_data = sqlx::query!("SELECT name, email, status FROM subscriptions",)
        .fetch_one(&app.db_pool)
        .await
        .expect("Failed to fetch subscriber from the database");

    assert_eq!(db_data.email, "ursula_le_guin@gmail.com");
    assert_eq!(db_data.name, "le guin");
    assert_eq!(db_data.status, "confirmed");
}

#[actix_web::test]
async fn test_confirmed_subscribers_are_cleared_from_subscription_tokens_table() {
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    app.send_subscription_request(body.into()).await;
    app.send_subscription_request(body.into()).await;

    let pending_subscriptors_count = sqlx::query!("SELECT COUNT(*) FROM subscription_tokens",)
        .fetch_one(&app.db_pool)
        .await
        .unwrap();
    assert_eq!(pending_subscriptors_count.count.unwrap(), 2);

    let email_client_response = &app.email_server.received_requests().await.unwrap()[0];
    let confirmation_link = app.get_confirmation_link(email_client_response);

    let response = reqwest::get(confirmation_link.text_link).await.unwrap();
    assert_eq!(response.status().as_u16(), 200);

    let pending_subscriptors_count = sqlx::query!("SELECT COUNT(*) FROM subscription_tokens",)
        .fetch_one(&app.db_pool)
        .await
        .unwrap();
    assert_eq!(pending_subscriptors_count.count.unwrap(), 0);
}
