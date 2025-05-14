use tracing_error::InstrumentError;

#[derive(thiserror::Error, Debug)]
pub enum ErrorKind {
	#[error("interpulse error: {0}")]
	Interpulse(#[from] interpulse::Error),
	#[error("invalid input: {0}")]
	InvalidInput(String),
	#[error("failed to manage asynchronous tasks")]
	TaskError(#[from] tokio::task::JoinError),
	#[error("failed to deserialize JSON: {0}")]
	SerdeJSON(#[from] serde_json::Error),
	#[error("failed to deserialize XML: {0}")]
	SerdeXML(#[from] serde_xml_rs::Error),
	#[error("failed to fetch {item}")]
	Fetch { inner: reqwest::Error, item: String },
	#[error("failed to acquire semaphore: {0}")]
	Acquire(#[from] tokio::sync::AcquireError),
	#[error("tracing error: {0}")]
	Tracing(#[from] tracing::subscriber::SetGlobalDefaultError),
	#[error("zip error: {0}")]
	Zip(#[from] async_zip::error::ZipError),
	#[error("failed to upload file to S3: {file}")]
	S3 {
		inner: Box<s3::error::S3Error>,
		file: String,
	},
	#[error("failed to validate file checksum at url {url} with hash {hash} after {tries} tries")]
	ChecksumFailure {
		hash: String,
		url: String,
		tries: u32,
	},
}

#[derive(Debug)]
pub struct Error {
	pub source: tracing_error::TracedError<ErrorKind>,
}

impl std::fmt::Display for Error {
	fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(fmt, "{}", self.source)
	}
}

impl<E: Into<ErrorKind>> From<E> for Error {
	fn from(source: E) -> Self {
		let error = Into::<ErrorKind>::into(source);
		Self {
			source: error.in_current_span(),
		}
	}
}

impl ErrorKind {
	#[must_use]
	pub fn as_error(self) -> Error {
		self.into()
	}
}

pub type Result<T> = core::result::Result<T, Error>;
