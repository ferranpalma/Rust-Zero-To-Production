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
    skip(form, db_pool),
    fields(
        subscriber_name = %form.name,
        subscriber_email = %form.email
    )
)]
pub async fn subscribe(form: web::Form<FormData>, db_pool: web::Data<PgPool>) -> HttpResponse {
    let subscriber = match form.0.try_into() {
        Ok(subscriber) => subscriber,
        Err(_) => return HttpResponse::BadRequest().finish(),
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
        INSERT INTO subscriptions (id, email, name, subscribed_at, status)
        VALUES ($1, $2, $3, $4, 'confirmed')
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
