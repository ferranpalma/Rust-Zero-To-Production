use wiremock::{
    matchers::{any, method, path},
    Mock, ResponseTemplate,
};

use crate::helpers::{spawn_app, ConfirmationLink, TestingApp};

#[actix_web::test]
async fn test_mails_are_delivered_to_confirmed_subscribers() {
    let app = spawn_app().await;
    create_confirmed_subscriber(&app).await;

    // Mock Postmark and assert no requests are fired
    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    // Act
    let newsletter_body = serde_json::json!({
        "title": "Newsletter title",
        "content": {
            "text": "Newsletter body as plain text",
            "html": "<p>Newsletter body as HTML</p>",
        }
    });
    let response = reqwest::Client::new()
        .post(format!("{}/newsletters", &app.web_address))
        .json(&newsletter_body)
        .send()
        .await
        .expect("Failed to execute request");
    assert_eq!(response.status().as_u16(), 200);
}

#[actix_web::test]
async fn test_mails_are_not_delivered_to_unconfirmed_subscribers() {
    let app = spawn_app().await;
    let _ = create_unconfirmed_subscriber(&app).await;

    // Mock Postmark and assert no requests are fired
    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&app.email_server)
        .await;

    // Act
    let newsletter_body = serde_json::json!({
        "title": "Newsletter title",
        "content": {
            "text": "Newsletter body as plain text",
            "html": "<p>Newsletter body as HTML</p>",
        }
    });
    let response = reqwest::Client::new()
        .post(format!("{}/newsletters", &app.web_address))
        .json(&newsletter_body)
        .send()
        .await
        .expect("Failed to execute request");
    assert_eq!(response.status().as_u16(), 200);
}

async fn create_unconfirmed_subscriber(app: &TestingApp) -> ConfirmationLink {
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    let _mock_guard = Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .named("Create unconfirmed subscriber")
        .expect(1)
        .mount_as_scoped(&app.email_server)
        .await;

    app.send_subscription_request(body.into())
        .await
        .error_for_status()
        .unwrap();

    let email_server_response = &app
        .email_server
        .received_requests()
        .await
        .unwrap()
        .pop()
        .unwrap();

    app.get_confirmation_link(email_server_response)
}

async fn create_confirmed_subscriber(app: &TestingApp) {
    let confirmation_link = create_unconfirmed_subscriber(app).await;

    reqwest::get(confirmation_link.html_link)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
}
