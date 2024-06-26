use uuid::Uuid;
use wiremock::{
    matchers::{any, method, path},
    Mock, ResponseTemplate,
};

use crate::helpers::{spawn_app, ConfirmationLink, TestingApp};

#[actix_web::test]
async fn test_requests_missing_authorization_are_rejected() {
    let app = spawn_app().await;

    let response = reqwest::Client::new()
        .post(&format!("{}/newsletters", &app.web_address))
        .json(&serde_json::json!({
            "title": "Newsletter title",
            "content": {
                "text": "Newsletter body as plain text",
                "html": "<p>Newsletter body as HTML</p>"
            }
        }))
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status().as_u16(), 401);
    assert_eq!(
        r#"Basic realm="publish""#,
        response.headers()["WWW-Authenticate"]
    );
}

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
    let response = app.send_newsletter(newsletter_body).await;
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
    let response = app.send_newsletter(newsletter_body).await;
    assert_eq!(response.status().as_u16(), 200);
}

#[actix_web::test]
async fn test_newsletter_with_invalid_data_returns_400() {
    let app = spawn_app().await;
    let cases = vec![
        (
            serde_json::json!({
                "content": {
                    "text": "Newsletter body as plain text",
                    "html": "<p>Newsletter body as HTML</p>",
                }
            }),
            "missing title",
        ),
        (
            serde_json::json!({
                "title": "Newsletter"
            }),
            "missing content",
        ),
    ];

    for (case, error) in cases {
        let response = app.send_newsletter(case).await;
        assert_eq!(
            response.status().as_u16(),
            400,
            "The API did not fail with HTTP400 when the payload was {}",
            error
        );
    }
}

#[actix_web::test]
async fn test_non_existing_user_is_rejected() {
    let app = spawn_app().await;

    let username = Uuid::new_v4().to_string();
    let password = Uuid::new_v4().to_string();
    assert_ne!(app.test_user.username, username);

    let response = reqwest::Client::new()
        .post(&format!("{}/newsletters", &app.web_address))
        .basic_auth(username, Some(password))
        .json(&serde_json::json!({
                "title": "Newsletter title",
                "content": {
                    "text": "Newsletter body as plain text",
                    "html": "<p>Newsletter body as HTML</p>"
                }
        }))
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status().as_u16(), 401);
    assert_eq!(
        r#"Basic realm="publish""#,
        response.headers()["WWW-Authenticate"]
    );
}

#[actix_web::test]
async fn test_user_with_invalid_password_is_rejected() {
    let app = spawn_app().await;

    let username = &app.test_user.username;
    let password = Uuid::new_v4().to_string();
    assert_ne!(app.test_user.password, password);

    let response = reqwest::Client::new()
        .post(&format!("{}/newsletters", &app.web_address))
        .basic_auth(username, Some(password))
        .json(&serde_json::json!({
                "title": "Newsletter title",
                "content": {
                    "text": "Newsletter body as plain text",
                    "html": "<p>Newsletter body as HTML</p>"
                }
        }))
        .send()
        .await
        .expect("Failed to execute request");

    assert_eq!(response.status().as_u16(), 401);
    assert_eq!(
        r#"Basic realm="publish""#,
        response.headers()["WWW-Authenticate"]
    );
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
