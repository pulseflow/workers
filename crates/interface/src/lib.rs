//! # Interpulse
//!
//! Interpulse is a library which provides models and methods to interact with Pulseflow apis

#![warn(missing_docs, unused_import_braces, missing_debug_implementations)]

/// Models and methods for fetching from Pulseflow
pub mod api;
/// Utilities and helper functions for APIs
pub mod utils;

#[derive(thiserror::Error, Debug)]
/// An error type representing possible errors when fetching metadata
pub enum Error {
	/// A checksum was failed to validate for a file or url
	#[error("failed to validate checksum at {url}#{hash} after {tries} attempts")]
	ChecksumFailure {
		/// The checksum's hash
		hash: String,
		/// The URL of the file attempted to be checked
		url: String,
		/// The amount of tries that the file was checked until failure
		tries: u32,
	},
	/// There was an error deserializing metadata
	#[error("failed to deserialize metadata json")]
	SerdeError(#[from] serde_json::Error),
	/// There was a network error when fetching an item
	#[error("failed to fetch the object {item}")]
	FetchError {
		/// The internal @network/reqwest Error
		inner: reqwest::Error,
		/// The object that failed to be fetched
		item: String,
	},
	/// There was an error when managing @async/tokio tasks
	#[error("failed to manage asynchronous tasks with tokio")]
	TaskError(#[from] tokio::task::JoinError),
	/// There was an error while parsing an input (catch-all)
	#[error("{0}")]
	ParseError(String),
}
