pub use crate::Error;

/// Prelude imports to avoid repetitive code
pub mod prelude;

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
