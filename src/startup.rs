use actix_web::{dev::Server, web, App, HttpServer};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::net::TcpListener;
use tracing_actix_web::TracingLogger;

use crate::{
    configuration::{DatabaseSettings, Settings},
    email_client::EmailClient,
    routes::{health_check, subscriptions},
};

pub struct Application {
    server: Server,
    port: u16,
}

impl Application {
    pub async fn build(configuration: Settings) -> Result<Self, std::io::Error> {
        let db_pool = Self::get_db_connection_pool(&configuration.database);

        let sender_email = configuration
            .email_client
            .sender()
            .expect("Invalid sender email address.");
        let http_client_timeout = configuration.email_client.timeout();
        let email_client = EmailClient::new(
            configuration.email_client.base_url,
            sender_email,
            configuration.email_client.authorization_token,
            http_client_timeout,
        );

        let address = format!(
            "{}:{}",
            configuration.application.address, configuration.application.port
        );
        let listener = TcpListener::bind(address)?;

        let port = listener.local_addr().unwrap().port();
        let server = Self::get_server(listener, db_pool, email_client)?;

        Ok(Self { server, port })
    }

    pub fn get_db_connection_pool(db_config: &DatabaseSettings) -> PgPool {
        PgPoolOptions::new()
            .acquire_timeout(std::time::Duration::from_secs(2))
            .connect_lazy_with(db_config.connect_database_instance())
    }

    pub fn get_application_port(&self) -> u16 {
        self.port
    }

    pub async fn run_application(self) -> Result<(), std::io::Error> {
        self.server.await
    }

    fn get_server(
        listener: TcpListener,
        db_pool: PgPool,
        email_client: EmailClient,
    ) -> Result<Server, std::io::Error> {
        // Wrap the pool in web::Data, that ends up as an Arc pointer
        let db_connection = web::Data::new(db_pool);
        let email_client = web::Data::new(email_client);
        let server = HttpServer::new(move || {
            App::new()
                .wrap(TracingLogger::default())
                .route("/health_check", web::get().to(health_check::health_check))
                .route("/subscriptions", web::post().to(subscriptions::subscribe))
                .app_data(db_connection.clone())
                .app_data(email_client.clone())
        })
        .listen(listener)?
        .run();

        Ok(server)
    }
}
