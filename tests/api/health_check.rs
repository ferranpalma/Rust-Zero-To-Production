use crate::helpers::spawn_app;

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
