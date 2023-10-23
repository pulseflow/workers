/// Sorting APIs for Metadata
pub mod api;
/// Utilities relating to sorting Metadata
pub mod utils;

#[derive(thiserror::Error, Debug)]
/// Core error object for Metadata sorters
pub enum Error {
	#[error("{0}")]
	InterpulseError(#[from] interpulse::Error),
	#[error("could not deserialize JSON")]
	SerdeError(#[from] serde_json::Error),
	#[error("could not deserialize XML")]
	XMLError(#[from] serde_xml_rs::Error),
	#[error("could not fetch {item}")]
	FetchError { inner: reqwest::Error, item: String },
	#[error("failure during tokio task")]
	TaskError(#[from] tokio::task::JoinError),
	#[error("could not upload to S3")]
	S3Error { inner: s3::error::S3Error, file: String },
	#[error("could not parse version as semver; {0}")]
	SemVerError(#[from] semver::Error),
	#[error("could not read zip file; {0}")]
	ZipError(#[from] zip::result::ZipError),
	#[error("could not read from @std/io; {0}")]
	IoError(#[from] std::io::Error),
	#[error("could not obtain arc reference")]
	ArcError,
	#[error("could not obtain semaphore reference; {0}")]
	AcquireError(#[from] tokio::sync::AcquireError),
}