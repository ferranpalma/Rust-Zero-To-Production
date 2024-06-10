use actix_web::{http::StatusCode, web, HttpResponse, ResponseError};
use anyhow::Context;
use askama_actix::Template;
use chrono::Utc;
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use sqlx::{Executor, PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::{
    email_client::EmailClient,
    startup::ApplicationBaseUrl,
    types::{templates::ConfirmationEmailTemplate, Subscriber, SubscriberEmail, SubscriberName},
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

fn generate_token() -> String {
    let mut rng = thread_rng();
    std::iter::repeat_with(|| rng.sample(Alphanumeric))
        .map(char::from)
        .take(25)
        .collect()
}

#[derive(thiserror::Error)]
pub enum SubscribeError {
    #[error("{0}")]
    ValidationError(String),
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl std::fmt::Debug for SubscribeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl ResponseError for SubscribeError {
    fn status_code(&self) -> StatusCode {
        match self {
            SubscribeError::ValidationError(_) => StatusCode::BAD_REQUEST,
            SubscribeError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
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
) -> Result<HttpResponse, SubscribeError> {
    let subscriber = form.0.try_into().map_err(SubscribeError::ValidationError)?;

    let mut db_transaction = db_pool
        .begin()
        .await
        .context("Failed to acquire a Postgres connection from the pool.")?;
    let subscriber_id = insert_susbcriber_db(&mut db_transaction, &subscriber)
        .await
        .context("Failed to insert new subscriber in the database.")?;

    let subscriber_token = generate_token();
    store_subscriber_token(&mut db_transaction, subscriber_id, &subscriber_token)
        .await
        .context("Failed to store the confirmation token for a new subscriber.")?;

    db_transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction for a new subscriber.")?;

    send_confirmation_email(
        &email_client,
        subscriber,
        &application_base_url.0,
        &subscriber_token,
    )
    .await
    .context("Failed to send confirmation email.")?;

    Ok(HttpResponse::Ok().finish())
}

pub struct StoreTokenError(sqlx::Error);

impl std::fmt::Debug for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

impl std::fmt::Display for StoreTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "A database error was encountered while trying to store a subscription token."
        )
    }
}

pub fn error_chain_fmt(
    e: &impl std::error::Error,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    writeln!(f, "{}\n", e)?;
    let mut current = e.source();
    while let Some(cause) = current {
        writeln!(f, "Caused by:\n\t{}", cause)?;
        current = cause.source();
    }

    Ok(())
}

impl std::error::Error for StoreTokenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

#[tracing::instrument(
    name = "Store subscription token in the database",
    skip(subscription_token, db_transaction)
)]
async fn store_subscriber_token(
    db_transaction: &mut Transaction<'_, Postgres>,
    subscriber_id: Uuid,
    subscription_token: &str,
) -> Result<(), StoreTokenError> {
    let query = sqlx::query!(
        r#"
            INSERT INTO subscription_tokens (subscription_token, subscriber_id) 
            VALUES ($1, $2)
        "#,
        subscription_token,
        subscriber_id
    );
    db_transaction
        .execute(query)
        .await
        .map_err(StoreTokenError)?;

    Ok(())
}

#[tracing::instrument(
    name = "Save subscriber details in the database",
    skip(db_transaction, subscriber)
)]
async fn insert_susbcriber_db(
    db_transaction: &mut Transaction<'_, Postgres>,
    subscriber: &Subscriber,
) -> Result<Uuid, sqlx::Error> {
    let subscriber_id = Uuid::new_v4();
    let query = sqlx::query!(
        r#"
        INSERT INTO subscriptions (id, email, name, subscribed_at, status)
        VALUES ($1, $2, $3, $4, 'pending_confirmation')
        ON CONFLICT (email) 
        DO UPDATE SET
           id = EXCLUDED.id
        WHERE subscriptions.status = 'pending_confirmation'
        "#,
        subscriber_id,
        subscriber.email.as_ref(),
        subscriber.name.as_ref(),
        Utc::now()
    );
    db_transaction.execute(query).await?;

    Ok(subscriber_id)
}

#[tracing::instrument(
    name = "Send a confirmation email to a new subscriber",
    skip(email_client, subscriber, application_base_url, subscription_token)
)]
async fn send_confirmation_email(
    email_client: &EmailClient,
    subscriber: Subscriber,
    application_base_url: &String,
    subscription_token: &str,
) -> Result<(), reqwest::Error> {
    let confirmation_link = format!(
        "{}/subscriptions/confirm?subscription_token={}",
        application_base_url, subscription_token
    );

    let html_body = ConfirmationEmailTemplate {
        confirmation_link: &confirmation_link,
    };
    let html_body = html_body
        .render()
        .expect("Failed to render html for confirmation email");

    let text_body = format!(
        "Welcom to our newsletter!\nVisit {} to confirm your subscription",
        confirmation_link
    );

    email_client
        .send_email(&subscriber.email, "Welcome!", &html_body, &text_body)
        .await
}
