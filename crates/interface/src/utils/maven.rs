use crate::Error;

/// Converts a Maven artifact to a Path
pub fn get_path_from_artifact(artifact: &str) -> Result<String, Error> {
	let name_items = artifact.split(':').collect::<Vec<&str>>();

	let package = name_items.first().ok_or_else(|| {
		Error::ParseError(format!("unable to find package for library {}", &artifact))
	})?;
	let name = name_items.get(1).ok_or_else(|| {
		Error::ParseError(format!("unable to find name for library {}", &artifact))
	})?;

	if name_items.len() == 3 {
		let versions_ext = name_items
			.get(2)
			.ok_or_else(|| {
				Error::ParseError(format!("unable to find version for library {}", &artifact))
			})?
			.split('@')
			.collect::<Vec<&str>>();
		let version = versions_ext.first().ok_or_else(|| {
			Error::ParseError(format!("unable to find version for library {}", &artifact))
		})?;

		Ok(format!(
			"{}/{}/{}/{}-{}.{}",
			package.replace('.', "/"),
			name,
			version,
			name,
			version,
			versions_ext.get(1).unwrap_or(&"jar")
		))
	} else {
		let version = name_items.get(2).ok_or_else(|| {
			Error::ParseError(format!("unable to find version for library {}", &artifact))
		})?;
		let data_ext = name_items
			.get(3)
			.ok_or_else(|| {
				Error::ParseError(format!("unable to find data for library {}", &artifact))
			})?
			.split('@')
			.collect::<Vec<&str>>();
		let data = data_ext.first().ok_or_else(|| {
			Error::ParseError(format!("unable to find data for library {}", &artifact))
		})?;

		Ok(format!(
			"{}/{}/{}/{}-{}-{}.{}",
			package.replace('.', "/"),
			name,
			version,
			name,
			version,
			data,
			data_ext.get(1).unwrap_or(&"jar")
		))
	}
}

/// Downloads a file from specified mirrrors
pub async fn download_file_mirrors(
	base: &str,
	mirrors: &[&str],
	sha1: Option<&str>,
) -> Result<bytes::Bytes, Error> {
	if mirrors.is_empty() {
		return Err(Error::ParseError("no mirrors provided".to_string()));
	}

	for (index, mirror) in mirrors.iter().enumerate() {
		let result = download_file(&format!("{}{}", mirror, base), sha1).await;
		if result.is_ok() || (result.is_err() && index == (mirrors.len() - 1)) {
			return result;
		}
	}

	unreachable!()
}

/// Downloads a file with retry and checksum functionality
pub async fn download_file(url: &str, sha1: Option<&str>) -> Result<bytes::Bytes, Error> {
	let mut headers = reqwest::header::HeaderMap::new();
	if let Ok(header) = reqwest::header::HeaderValue::from_str(&format!(
		"pulseflow/workers/{} (support@dyn.gay)",
		env!("CARGO_PKG_VERSION")
	)) {
		headers.insert(reqwest::header::USER_AGENT, header);
	}

	let client = reqwest::Client::builder()
		.tcp_keepalive(Some(std::time::Duration::from_secs(10)))
		.timeout(std::time::Duration::from_secs(15))
		.default_headers(headers)
		.build()
		.map_err(|err| Error::FetchError {
			inner: err,
			item: url.to_string(),
		})?;

	for attempt in 1..=4 {
		let result = client.get(url).send().await;

		match result {
			Ok(x) => {
				let bytes = x.bytes().await;

				if let Ok(bytes) = bytes {
					if let Some(sha1) = sha1 {
						if &*get_hash(bytes.clone()).await? != sha1 {
							if attempt <= 3 {
								continue;
							} else {
								return Err(Error::ChecksumFailure {
									hash: sha1.to_string(),
									url: url.to_string(),
									tries: attempt,
								});
							}
						}
					}

					return Ok(bytes);
				} else if attempt <= 3 {
					continue;
				} else if let Err(err) = bytes {
					return Err(Error::FetchError {
						inner: err,
						item: url.to_string(),
					});
				}
			}
			Err(_) if attempt <= 3 => continue,
			Err(err) => {
				return Err(Error::FetchError {
					inner: err,
					item: url.to_string(),
				})
			}
		}
	}

	unreachable!()
}

/// Computes a checksum of the input Bytes
pub async fn get_hash(bytes: bytes::Bytes) -> Result<String, Error> {
	let hash = tokio::task::spawn_blocking(|| sha1_smol::Sha1::from(bytes).hexdigest()).await?;

	Ok(hash)
}
