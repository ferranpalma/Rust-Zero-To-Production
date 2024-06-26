use argon2::{password_hash::SaltString, Algorithm, Argon2, Params, PasswordHasher, Version};
use once_cell::sync::Lazy;
use sqlx::{Connection, Executor, PgConnection, PgPool};
use uuid::Uuid;
use wiremock::MockServer;

use zero2prod::{
    configuration::{get_configuration, DatabaseSettings},
    startup::Application,
    telemetry,
};

static TRACING: Lazy<()> = Lazy::new(|| {
    if std::env::var("TEST_LOG").is_ok() {
        telemetry::Telemetry::create("test".into(), "debug".into(), std::io::stdout);
    } else {
        telemetry::Telemetry::create("test".into(), "debug".into(), std::io::sink);
    }
});

pub struct TestingApp {
    pub web_address: String,
    pub db_pool: PgPool,
    pub email_server: MockServer,
    pub port: u16,
    pub test_user: TestUser,
}

#[derive(Debug)]
pub struct ConfirmationLink {
    pub html_link: reqwest::Url,
    pub text_link: reqwest::Url,
}

impl TestingApp {
    pub async fn send_subscription_request(&self, body: String) -> reqwest::Response {
        reqwest::Client::new()
            .post(&format!("{}/subscriptions", &self.web_address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("Failed to execute request")
    }

    pub async fn send_newsletter(&self, body: serde_json::Value) -> reqwest::Response {
        reqwest::Client::new()
            .post(&format!("{}/newsletters", &self.web_address))
            .basic_auth(&self.test_user.username, Some(&self.test_user.password))
            .json(&body)
            .send()
            .await
            .expect("Failed to execute request")
    }

    pub fn get_confirmation_link(
        &self,
        email_client_response: &wiremock::Request,
    ) -> ConfirmationLink {
        let response_body: serde_json::Value =
            serde_json::from_slice(&email_client_response.body).unwrap();

        let get_link = |s: &str| {
            let links: Vec<_> = linkify::LinkFinder::new()
                .links(s)
                .filter(|link| *link.kind() == linkify::LinkKind::Url)
                .collect();
            assert_eq!(links.len(), 1);

            let raw_confirmation_link = links[0].as_str().to_owned();
            let mut confirmation_link = reqwest::Url::parse(&raw_confirmation_link).unwrap();

            assert_eq!(confirmation_link.host_str().unwrap(), "127.0.0.1");

            confirmation_link.set_port(Some(self.port)).unwrap();
            confirmation_link
        };

        let html_link = get_link(response_body["HtmlBody"].as_str().unwrap());
        let text_link = get_link(response_body["TextBody"].as_str().unwrap());

        ConfirmationLink {
            html_link,
            text_link,
        }
    }
}

pub async fn spawn_app() -> TestingApp {
    Lazy::force(&TRACING);

    let email_server = MockServer::start().await;

    let configuration = {
        let mut config = get_configuration().expect("Failed to read configuration");
        // Create a different db name for each test so every test is db isolated
        config.database.name = Uuid::new_v4().to_string();
        config.application.port = 0;
        config.email_client.base_url = email_server.uri();

        config
    };

    let _ = configure_testing_database(&configuration.database).await;

    let application = Application::build(configuration.clone())
        .await
        .expect("Failed to build application server.");
    let web_address = format!("http://127.0.0.1:{}", application.get_application_port());
    let port = application.get_application_port();

    let _ = tokio::spawn(application.run_application());

    let testing_app = TestingApp {
        web_address,
        db_pool: Application::get_db_connection_pool(&configuration.database),
        email_server,
        port,
        test_user: TestUser::generate(),
    };

    testing_app
        .test_user
        .store_in_db(&testing_app.db_pool)
        .await;

    testing_app
}

async fn configure_testing_database(config: &DatabaseSettings) -> PgPool {
    // Connect to postgres, not to a specific postgres database
    let mut db_connection = PgConnection::connect_with(&config.connect_database_engine())
        .await
        .expect("Failed to connect to Postgres");

    // Create and migrate the database
    db_connection
        .execute(format!(r#"CREATE DATABASE "{}";"#, config.name).as_str())
        .await
        .expect("Failed to create database");

    let db_pool = PgPool::connect_with(config.connect_database_instance())
        .await
        .expect("Failed to connect to Postgres");

    sqlx::migrate!("./migrations")
        .run(&db_pool)
        .await
        .expect("Failed to migrate database");

    db_pool
}

pub struct TestUser {
    pub user_id: Uuid,
    pub username: String,
    pub password: String,
}

impl TestUser {
    pub fn generate() -> Self {
        Self {
            user_id: Uuid::new_v4(),
            username: Uuid::new_v4().to_string(),
            password: Uuid::new_v4().to_string(),
        }
    }

    async fn store_in_db(&self, db_pool: &PgPool) {
        let salt = SaltString::generate(&mut rand::thread_rng());
        let password_hash = Argon2::new(
            Algorithm::Argon2id,
            Version::V0x13,
            Params::new(15000, 2, 1, None).unwrap(),
        )
        .hash_password(self.password.as_bytes(), &salt)
        .unwrap()
        .to_string();
        sqlx::query!(
            "INSERT INTO users (user_id, username, password_hash) VALUES ($1, $2, $3)",
            self.user_id,
            self.username,
            password_hash
        )
        .execute(db_pool)
        .await
        .expect("Failed to store the test user");
    }
}
