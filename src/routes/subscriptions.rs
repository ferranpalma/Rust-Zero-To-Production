use actix_web::{web, HttpResponse};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    email_client::EmailClient,
    startup::ApplicationBaseUrl,
    types::{Subscriber, SubscriberEmail, SubscriberName},
};

#[derive(serde::Deserialize)]
pub struct FormData {
    email: String,
    name: String,
}

impl TryInto<Subscriber> for FormData {
    type Error = String;
    fn try_into(self) -> Result<Subscriber, Self::Error> {
        let subscriber_name = SubscriberName::parse(self.name)?;
        let subscriber_email = SubscriberEmail::parse(self.email)?;
        Ok(Subscriber {
            email: subscriber_email,
            name: subscriber_name,
        })
    }
}

#[tracing::instrument(
    name = "Add new subscriber",
    skip(form, db_pool, email_client, application_base_url),
    fields(
        subscriber_name = %form.name,
        subscriber_email = %form.email
    )
)]
pub async fn subscribe(
    form: web::Form<FormData>,
    db_pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    application_base_url: web::Data<ApplicationBaseUrl>,
) -> HttpResponse {
    let subscriber = match form.0.try_into() {
        Ok(subscriber) => subscriber,
        Err(_) => return HttpResponse::BadRequest().finish(),
    };

    if insert_susbcriber_db(&db_pool, &subscriber).await.is_err() {
        return HttpResponse::InternalServerError().finish();
    }

    if send_confirmation_email(&email_client, subscriber, &application_base_url.0)
        .await
        .is_err()
    {
        return HttpResponse::InternalServerError().finish();
    }

    HttpResponse::Ok().finish()
}

#[tracing::instrument(
    name = "Save subscriber details in the database",
    skip(db_pool, subscriber)
)]
async fn insert_susbcriber_db(
    db_pool: &PgPool,
    subscriber: &Subscriber,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        INSERT INTO subscriptions (id, email, name, subscribed_at, status)
        VALUES ($1, $2, $3, $4, 'pending_confirmation')
        "#,
        Uuid::new_v4(),
        subscriber.email.as_ref(),
        subscriber.name.as_ref(),
        Utc::now()
    )
    .execute(db_pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;

    Ok(())
}

#[tracing::instrument(
    name = "Send a confirmation email to a new subscriber",
    skip(email_client, subscriber, application_base_url)
)]
async fn send_confirmation_email(
    email_client: &EmailClient,
    subscriber: Subscriber,
    application_base_url: &String,
) -> Result<(), reqwest::Error> {
    let confirmation_link = format!(
        "{}/subscriptions/confirm?subscription_token=mytoken",
        application_base_url
    );
    let html_body = format!(
        "Welcome to my newsletter! <br />\
            Click <a href=\"{}\">here</a> to confirm your subscription",
        confirmation_link
    );
    let text_body = format!(
        "Welcom to our newsletter!\nVisit {} to confirm your subscription",
        confirmation_link
    );

    email_client
        .send_email(subscriber.email, "Welcome!", &html_body, &text_body)
        .await
}
