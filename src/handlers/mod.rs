use axum::{extract::Form, response::Response};
use serde::Deserialize;

use mdict_rs::lucky;
use mdict_rs::query::query;

#[derive(Deserialize)]
pub struct SearchQuery {
    word: String,
}

pub async fn handle_query(Form(input): Form<SearchQuery>) -> Response {
    axum::http::Response::builder()
        .header("Content-Type", "text/plain")
        .body(query(input.word).into())
        .unwrap()
}

pub async fn handle_lucky() -> Response {
    axum::http::Response::builder()
        .header("Content-Type", "text/plain")
        .body(query(lucky::lucky_word()).into())
        .unwrap()
}
