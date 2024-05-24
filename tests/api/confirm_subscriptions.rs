use crate::helpers::spawn_app;

#[actix_web::test]
async fn test_requests_without_token_are_rejected() {
    let app = spawn_app().await;

    let response = reqwest::get(&format!("{}/subscriptions/confirm", app.web_address))
        .await
        .unwrap();

    assert_eq!(response.status().as_u16(), 400);
}
