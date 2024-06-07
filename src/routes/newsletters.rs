use actix_web::{http::StatusCode, web, HttpResponse, ResponseError};
use sqlx::PgPool;

use crate::routes::error_chain_fmt;

#[derive(serde::Deserialize)]
pub struct EmailData {
    title: String,
    content: Content,
}

#[derive(serde::Deserialize)]
pub struct Content {
    html: String,
    text: String,
}

struct ConfirmedSubscriber {
    email: String,
}

#[derive(thiserror::Error)]
pub enum PublishError {
    #[error(transparent)]
    UnexpectedError(#[from] anyhow::Error),
}

impl ResponseError for PublishError {
    fn status_code(&self) -> StatusCode {
        match self {
            PublishError::UnexpectedError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl std::fmt::Debug for PublishError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        error_chain_fmt(self, f)
    }
}

pub async fn publish_newsletter(
    db_pool: web::Data<PgPool>,
    _body: web::Json<EmailData>,
) -> Result<HttpResponse, PublishError> {
    let _confirmed_subscribers = get_confirmed_subscribers(&db_pool).await?;
    Ok(HttpResponse::Ok().finish())
}

#[tracing::instrument(name = "Get confirmed subscribers", skip(db_pool))]
async fn get_confirmed_subscribers(
    db_pool: &PgPool,
) -> Result<Vec<ConfirmedSubscriber>, anyhow::Error> {
    let confirmed_subscribers = sqlx::query_as!(
        ConfirmedSubscriber,
        r#"
            SELECT email
            FROM subscriptions
            WHERE status = 'confirmed'
        "#
    )
    .fetch_all(db_pool)
    .await?;

    Ok(confirmed_subscribers)
}
