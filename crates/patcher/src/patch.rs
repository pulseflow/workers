use interpulse::api::minecraft::{Library, PartialLibrary};
use interpulse::utils::prelude::*;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
/// A structure used to represent a patch
pub struct LibraryPatch {
	#[serde(rename = "_comment")]
	#[allow(clippy::pub_underscore_fields)]
	pub _comment: String,
	#[serde(rename = "match")]
	pub match_: Vec<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub additional_libraries: Option<Vec<Library>>,
	#[serde(rename = "override")]
	#[serde(skip_serializing_if = "Option::is_none")]
	pub override_: Option<PartialLibrary>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub patch_additional_libraries: Option<bool>,
}

pub fn collect_patch_files(dir: &String, dest: &String) -> anyhow::Result<()> {
	let dir = std::path::Path::new(&dir);
	let dest = std::path::Path::new(&dest);

	let patches: Vec<LibraryPatch> = std::fs::read_dir(dir)?
		.filter_map(Result::ok)
		.filter(|p| p.path().extension().map_or(false, |e| e == "json"))
		.filter_map(|p| serde_json::from_str(&std::fs::read_to_string(p.path()).ok()?).ok())
		.collect();

	std::fs::write(dest, serde_json::to_string(&patches)?)?;

	Ok(())
}

pub fn uncollect_patch_files(dir: &String, dest: &String) -> anyhow::Result<()> {
	let dir = std::path::Path::new(&dir);
	let dest = std::path::Path::new(&dest);

	let data = std::fs::read_to_string(dest)?;
	let patches: Vec<LibraryPatch> = serde_json::from_str(&data)?;

	let mut file_names: HashMap<String, usize> = HashMap::new();
	let mut result: Vec<(String, LibraryPatch)> = Vec::new();

	for patch in &patches {
		if let Some(spliced) = patch.match_.first().and_then(|m| m.split(':').nth(1)) {
			result.push((
				generate_unique_file_name(&mut file_names, spliced),
				patch.clone(),
			));
		}
	}

	for (file_name, patch) in result {
		let output_path = dir.join(file_name);
		let file_writer = std::io::BufWriter::new(std::fs::File::create(output_path)?);
		let formatter = serde_json::ser::PrettyFormatter::with_indent(b"	");
		let mut serializer = serde_json::Serializer::with_formatter(file_writer, formatter);
		patch.serialize(&mut serializer)?;
	}

	Ok(())
}

fn generate_unique_file_name(file_names: &mut HashMap<String, usize>, base_name: &str) -> String {
	let count = file_names.entry(base_name.to_string()).or_insert(0);
	*count += 1;

	if *count == 1 {
		format!("{}.json", base_name)
	} else {
		format!("{base_name}-{count}.json")
	}
}
