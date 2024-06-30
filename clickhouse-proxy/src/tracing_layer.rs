use std::time::Duration;

use axum::http::Request;
use axum::response::Response;
use tracing::*;

pub fn add_layer(router: axum::Router) -> axum::Router {
    router.layer(tower_http::trace::TraceLayer::new_for_http()
                .make_span_with(|request: &axum::http::Request<_>| {
let id = uuid::Uuid::new_v4();
                    let path = request
                        .extensions()
                        .get::<axum::extract::MatchedPath>()
                        .map(axum::extract::MatchedPath::as_str);
                    info_span!(
                        "request",
                        uuid= id.to_string(),
                        method = ?request.method(),
                        agent = request.headers().get(axum::http::header::USER_AGENT).and_then(|v| v.to_str().ok() ).unwrap_or_default(),
                        path,
                        some_other_field = tracing::field::Empty,
                    )
                })
                .on_request(|_request: &Request<_>, _span: &Span| {
                    info!("Serving request");
                })
                .on_response(|response: &Response, latency: Duration, _span: &Span| {
                    let code =response.status();
                    if code.is_client_error() {
                        warn!( ?latency, ?code, "Sending response with client error");
                    } else if code.is_server_error() {
                        error!( ?latency, ?code, "Sending response with server error");
                    } else {
                        info!( ?latency, ?code, "Sending response");
                    };
                })
                .on_body_chunk(|_chunk: &axum::body::Bytes, _latency: Duration, _span: &Span| {})
                .on_eos(
                    |_trailers: Option<&axum::http::HeaderMap>,
                     _stream_duration: Duration,
                     _span: &Span| {},
                )
                .on_failure(
                    |error: tower_http::classify::ServerErrorsFailureClass,
                     latency: Duration,
                     _span: &Span| {
                        warn!(?error, ?latency, "Encountered error");
                    },
                ))
}
