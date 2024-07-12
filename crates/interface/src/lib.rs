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
	/// There was an error while parsing an input (catch-all)
	#[error("{0}")]
	ParseError(String),
	/// Mirrors failed to download
	#[error("mirrors failed to be downloaded")]
	MirrorsFailed(),
}
