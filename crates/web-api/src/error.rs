//! Application error type and HTTP mapping.
//!
//! [`AppError`] wraps `pixiekit_core::Error` plus a few API-specific variants
//! and implements [`axum::response::IntoResponse`] so handlers can `?`-bubble
//! errors and let axum return a JSON body with the right status code.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use thiserror::Error;

use pixiekit_core::Error as CoreError;

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Core(#[from] CoreError),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Multipart error: {0}")]
    Multipart(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Image error: {0}")]
    Image(#[from] image::ImageError),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl AppError {
    pub fn status(&self) -> StatusCode {
        match self {
            AppError::BadRequest(_) | AppError::Multipart(_) => StatusCode::BAD_REQUEST,
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Image(_) | AppError::Json(_) => StatusCode::BAD_REQUEST,
            AppError::Core(e) => map_core_error(e),
            AppError::Io(_) | AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status();
        let body = json!({ "error": self.to_string() });
        (status, Json(body)).into_response()
    }
}

pub type AppResult<T> = std::result::Result<T, AppError>;

/// Map a `pixiekit_core::Error` to the appropriate HTTP status.
///
/// Phase 3 will add a `VtracerFailed` variant when `core::vectorize` lands.
/// When that merge happens, add the new arm here (likely `422
/// UNPROCESSABLE_ENTITY`) — it will surface as a non-exhaustive-match error
/// from rustc, so it cannot silently regress.
fn map_core_error(e: &CoreError) -> StatusCode {
    match e {
        CoreError::NotFound(_) => StatusCode::NOT_FOUND,
        CoreError::InvalidInput(_) => StatusCode::BAD_REQUEST,
        CoreError::Image(_) | CoreError::Json(_) => StatusCode::BAD_REQUEST,
        CoreError::FfmpegMissing => StatusCode::SERVICE_UNAVAILABLE,
        CoreError::FfmpegFailed { .. } => StatusCode::UNPROCESSABLE_ENTITY,
        CoreError::WebpEncode(_) => StatusCode::INTERNAL_SERVER_ERROR,
        CoreError::NoFrames(_) => StatusCode::UNPROCESSABLE_ENTITY,
        CoreError::InconsistentFrameSize { .. } => StatusCode::UNPROCESSABLE_ENTITY,
        CoreError::Io(_) | CoreError::Walk(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn core_not_found_maps_to_404() {
        let err = AppError::Core(CoreError::NotFound(PathBuf::from("/nope")));
        assert_eq!(err.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn core_invalid_input_maps_to_400() {
        let err = AppError::Core(CoreError::InvalidInput("bad".into()));
        assert_eq!(err.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn core_ffmpeg_missing_maps_to_503() {
        let err = AppError::Core(CoreError::FfmpegMissing);
        assert_eq!(err.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn core_no_frames_maps_to_422() {
        let err = AppError::Core(CoreError::NoFrames(PathBuf::from("/v.mp4")));
        assert_eq!(err.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[test]
    fn bad_request_maps_to_400() {
        let err = AppError::BadRequest("oops".into());
        assert_eq!(err.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn not_found_maps_to_404() {
        let err = AppError::NotFound("missing".into());
        assert_eq!(err.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn multipart_maps_to_400() {
        let err = AppError::Multipart("boundary missing".into());
        assert_eq!(err.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn internal_maps_to_500() {
        let err = AppError::Internal("boom".into());
        assert_eq!(err.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
