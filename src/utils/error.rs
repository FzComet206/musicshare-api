use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    LoginFail,
    SessionDeleteFailIdNotFound { id: u64 },
    SessionNotFound { id: u64 },
    WebRTCErr { source: String },
    PeerConnectionNotFound { peerid: String },
    LocalDescriptionMissing,
    YtDlError { source: String },
    AudioDownloadDirError { source: String },
    FileTooLarge { size: u64, limit: u64 },
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        println!("->> {:<12} - {self:?}", "INTO_RES");

        (StatusCode::INTERNAL_SERVER_ERROR, "UNHANDLED_CLIENT_ERROR").into_response()
    }
}

impl From<webrtc::Error> for Error {
    fn from(err: webrtc::Error) -> Self {
        Error::WebRTCErr {
            source: format!("{:?}", err),
        }
    }
}

impl From<yt_dlp::error::Error> for Error {
    fn from(err: yt_dlp::error::Error) -> Self {
        Error::YtDlError {
            source: format!("{:?}", err),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(_err: std::io::Error) -> Self {
        Error::AudioDownloadDirError {
            source: "Failed to create audio download directory".to_string(),
        }
    }
}