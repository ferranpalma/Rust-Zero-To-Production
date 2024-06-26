use actix_web::{http::StatusCode, web, HttpResponse, ResponseError};
use anyhow::Context;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{routes::error_chain_fmt, types::SubscriptionToken};

#[derive(serde::Deserialize)]
pub struct QueryParameters {
    pub subscription_token: String,
}

#[derive(thiserror::Error)]
pub enum ConfirmError {
    #[error("{0}")]
    ValidationError(String),
    #[error("There is no subscriber associated with the provided token.")]
    UnknownToken,
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl ResponseError for ConfirmError {
    fn status_code(&self) -> actix_web::http::StatusCode {
        match self {
            ConfirmError::ValidationError(_) => StatusCode::BAD_REQUEST,
            ConfirmError::UnknownToken => StatusCode::UNAUTHORIZED,
            ConfirmError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl std::fmt::Debug for ConfirmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

#[tracing::instrument(name = "Confirm a pending subscriber", skip(query_params, db_pool))]
pub async fn confirm(
    query_params: web::Query<QueryParameters>,
    db_pool: web::Data<PgPool>,
) -> Result<HttpResponse, ConfirmError> {
    let subscription_token = SubscriptionToken::parse(query_params.subscription_token.clone())
        .map_err(ConfirmError::ValidationError)?;

    let id = get_subscriber_id(&db_pool, subscription_token)
        .await
        .context("Failed to get the token's associated subscriber id.")?
        .ok_or(ConfirmError::UnknownToken)?;

    if subscriber_already_confirmed(&db_pool, id)
        .await
        .context("Failed to fetch the status of the token's associated subscriber")?
    {
        return Ok(HttpResponse::Ok().body("Already confirmed."));
    }

    confirm_subscriber(&db_pool, id)
        .await
        .context("Failed to update subscriber's status.")?;

    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(
    name = "Check the status of the subscriber associated to the token",
    skip(db_pool, subscriber_id)
)]
async fn subscriber_already_confirmed(
    db_pool: &PgPool,
    subscriber_id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query!(
        r#"SELECT status FROM subscriptions WHERE id = $1"#,
        subscriber_id
    )
    .fetch_one(db_pool)
    .await?;

    Ok(result.status == "confirmed")
}

#[tracing::instrument(name = "Mark subscriber as confirmed", skip(db_pool, subscriber_id))]
async fn confirm_subscriber(db_pool: &PgPool, subscriber_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"UPDATE subscriptions SET status = 'confirmed' WHERE id = $1"#,
        subscriber_id
    )
    .execute(db_pool)
    .await?;

    Ok(())
}

#[tracing::instrument(
    name = "Get subscriber_id from subscription_token",
    skip(db_pool, subscription_token)
)]
async fn get_subscriber_id(
    db_pool: &PgPool,
    subscription_token: SubscriptionToken,
) -> Result<Option<Uuid>, sqlx::Error> {
    let result = sqlx::query!(
        "SELECT subscriber_id FROM subscription_tokens WHERE subscription_token = $1",
        subscription_token.as_ref()
    )
    .fetch_optional(db_pool)
    .await?;

    Ok(result.map(|r| r.subscriber_id))
}
