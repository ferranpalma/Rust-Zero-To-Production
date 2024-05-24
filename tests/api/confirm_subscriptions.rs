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
    let response_body: serde_json::Value =
        serde_json::from_slice(&email_client_response.body).unwrap();

    let get_link = |s: &str| {
        let links: Vec<_> = linkify::LinkFinder::new()
            .links(s)
            .filter(|link| *link.kind() == linkify::LinkKind::Url)
            .collect();
        assert_eq!(links.len(), 1);

        links[0].as_str().to_owned()
    };

    let raw_confirmation_link = get_link(response_body["HtmlBody"].as_str().unwrap());
    let mut confirmation_link = Url::parse(&raw_confirmation_link).unwrap();

    assert_eq!(confirmation_link.host_str().unwrap(), "127.0.0.1");

    confirmation_link.set_port(Some(app.port)).unwrap();

    let response = reqwest::get(confirmation_link).await.unwrap();

    assert_eq!(response.status().as_u16(), 200);
}
