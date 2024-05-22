use zero2prod::{configuration, startup::Application, telemetry};

#[actix_web::main]
async fn main() -> Result<(), std::io::Error> {
    let configuration = configuration::get_configuration().expect("Failed to read configuration");

    telemetry::Telemetry::create("zero2prod".into(), "info".into(), std::io::stdout);

    let application = Application::build_server(configuration).await?;

    application.run_application().await?;

    Ok(())
}
