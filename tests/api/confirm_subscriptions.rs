use reqwest::Url;
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
