use crate::helpers::spawn_app;

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
        ("name=&email=", "missing name and email"),
        ("name=Ursula&email=definitely-not-an-email", "invalid email"),
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
