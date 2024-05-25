use actix_web::{web, HttpResponse};
use chrono::Utc;
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use sqlx::{Executor, PgPool, Postgres, Transaction};
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

fn generate_token() -> String {
    let mut rng = thread_rng();
    std::iter::repeat_with(|| rng.sample(Alphanumeric))
        .map(char::from)
        .take(25)
        .collect()
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

    let mut db_transaction = match db_pool.begin().await {
        Ok(db_transaction) => db_transaction,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    let subscriber_id = match insert_susbcriber_db(&mut db_transaction, &subscriber).await {
        Ok(id) => id,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    let subscriber_token = generate_token();

    if store_subscriber_token(&mut db_transaction, subscriber_id, &subscriber_token)
        .await
        .is_err()
    {
        tracing::error!("Error when storing the token in the database");
        return HttpResponse::InternalServerError().finish();
    }

    if send_confirmation_email(
        &email_client,
        subscriber,
        &application_base_url.0,
        &subscriber_token,
    )
    .await
    .is_err()
    {
        tracing::error!("Error when sending the confirmation email");
        return HttpResponse::InternalServerError().finish();
    }

    if db_transaction.commit().await.is_err() {
        tracing::error!("Unable to commit changes to the database");
        return HttpResponse::InternalServerError().finish();
    }

    HttpResponse::Ok().finish()
}

#[tracing::instrument(
    name = "Store subscription token in the database",
    skip(subscription_token, db_transaction)
)]
async fn store_subscriber_token(
    db_transaction: &mut Transaction<'_, Postgres>,
    subscriber_id: Uuid,
    subscription_token: &str,
) -> Result<(), sqlx::Error> {
    let query = sqlx::query!(
        r#"INSERT INTO subscription_tokens (subscription_token, subscriber_id) VALUES ($1, $2)"#,
        subscription_token,
        subscriber_id
    );
    db_transaction.execute(query).await.map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;

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
        "#,
        subscriber_id,
        subscriber.email.as_ref(),
        subscriber.name.as_ref(),
        Utc::now()
    );
    db_transaction.execute(query).await.map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;

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
