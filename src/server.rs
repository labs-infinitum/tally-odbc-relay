use std::sync::Arc;

use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use tokio::task::spawn_blocking;

use crate::api::{parse_query_sql, ErrorBody, HealthErr, HealthOk};
use crate::odbc::{execute_sql, ping_dsn};

#[derive(Clone)]
pub struct AppState {
    pub dsn: Arc<str>,
}

pub fn router(dsn: impl Into<String>) -> Router {
    let state = AppState {
        dsn: Arc::from(dsn.into()),
    };
    Router::new()
        .route("/health", get(health))
        .route("/query", post(query))
        .with_state(state)
}

async fn health(State(state): State<AppState>) -> Response {
    let dsn = state.dsn.clone();
    match spawn_blocking(move || ping_dsn(&dsn)).await {
        Ok(Ok(())) => (StatusCode::OK, Json(HealthOk { ok: true })).into_response(),
        Ok(Err(err)) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(HealthErr {
                ok: false,
                error: err.message(),
            }),
        )
            .into_response(),
        Err(err) => json_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

async fn query(State(state): State<AppState>, headers: HeaderMap, body: String) -> Response {
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok());
    let sql = match parse_query_sql(content_type, &body) {
        Ok(sql) => sql,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, err),
    };

    let dsn = state.dsn.clone();
    match spawn_blocking(move || execute_sql(&dsn, &sql)).await {
        Ok(Ok(result)) => (StatusCode::OK, Json(result)).into_response(),
        Ok(Err(err)) => json_error(StatusCode::BAD_GATEWAY, err.message()),
        Err(err) => json_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

fn json_error(status: StatusCode, error: impl Into<String>) -> Response {
    (
        status,
        Json(ErrorBody {
            error: error.into(),
        }),
    )
        .into_response()
}
