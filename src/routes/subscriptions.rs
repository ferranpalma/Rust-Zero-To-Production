use actix_web::{web, HttpResponse};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::types::{Subscriber, SubscriberEmail, SubscriberName};

#[derive(serde::Deserialize)]
pub struct FormData {
    email: String,
    name: String,
}

#[tracing::instrument(
    name = "Add new subscriber",
    skip(form, db_pool),
    fields(
        subscriber_name = %form.name,
        subscriber_email = %form.email
    )
)]
pub async fn subscribe(form: web::Form<FormData>, db_pool: web::Data<PgPool>) -> HttpResponse {
    let subscriber_name = match SubscriberName::parse(form.0.name) {
        Ok(s) => s,
        Err(_) => return HttpResponse::BadRequest().finish(),
    };
    let subscriber_email = match SubscriberEmail::parse(form.0.email) {
        Ok(s) => s,
        Err(_) => return HttpResponse::BadRequest().finish(),
    };
    let subscriber = Subscriber {
        // form.0 gives access to the underlying data in FormData
        email: subscriber_email,
        name: subscriber_name,
    };
    match insert_susbcriber_db(&db_pool, &subscriber).await {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(_) => HttpResponse::InternalServerError().finish(),
    }
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
        INSERT INTO subscriptions (id, email, name, subscribed_at)
        VALUES ($1, $2, $3, $4)
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
