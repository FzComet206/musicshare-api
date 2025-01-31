use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
pub type Result<T> = core::result::Result<T, Error>;
pub type ClientResult<T> = core::result::Result<T, ClientError>;

#[derive(Clone, Debug)]
pub enum Error {
    LoginFail,
    SessionDeleteFailIdNotFound { id: u64 },
    SessionNotFound { id: String },
    WebRTCErr { source: String },
    PeerConnectionNotFound { peerid: String },
    LocalDescriptionMissing,
    StdIoError { source: String },
    FileTooLarge { size: u64, limit: u64 },
    InvalidURL { url: String },
    LiveStreamNotSupported { url: String },
    PlayListParseErr { msg: String },
    DBConnectionFail { source: String },
    AuthFailNoToken,
    AuthFailInvalidToken,
    AuthFailCtxNotFound,
    DBError { source: String },
    ContentNotFound { msg: String },
    DownloadFailed { url: String },
    ConversionFailed { url: String },
    DatabaseWriteError { msg: String },
    UploadFailed { msg: String },
    QueueError { msg: String },
    DuplicateContent { msg: String },

    S3DownloadError { msg: String },
    S3LoadFileError { msg: String },
    ResetFileError { msg: String },

    SSEError { msg: String },

    BroadcasterError { msg: String },

    SessionExists,
}

#[derive(Clone, Debug)]
pub enum ClientError {
    SessionExists,
}


impl IntoResponse for Error {
    fn into_response(self) -> Response {
        println!("->> {:<12} - {self:?}", "INTO_RES");

        // return a response with the error code and message
        // return the error type and message
        (StatusCode::INTERNAL_SERVER_ERROR, format!("{:?}", self)).into_response()
    }
}

impl IntoResponse for ClientError {
    fn into_response(self) -> Response {
        println!("->> {:<12} - {self:?}", "INTO_RES");

        // return a response with the error code and message
        // return the error type and message
        (StatusCode::INTERNAL_SERVER_ERROR, format!("{:?}", self)).into_response()
    }
}

impl From<webrtc::Error> for Error {
    fn from(err: webrtc::Error) -> Self {
        Error::WebRTCErr {
            source: format!("{:?}", err),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(_err: std::io::Error) -> Self {
        Error::StdIoError {
            source: format!("{:?}", _err),
        }
    }
}

impl From<sqlx::Error> for Error {
    fn from(_err: sqlx::Error) -> Self {
        Error::DBConnectionFail {
            source: format!("{:?}", _err),
        }
    }

}