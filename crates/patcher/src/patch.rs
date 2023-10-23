use interpulse::{
	api::minecraft::{
		Library,
		PartialLibrary,
	},
	utils::prelude::*,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
/// A structure used to represent a patch
pub struct LibraryPatch {
	#[serde(rename = "_comment")]
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

pub fn collect_patch_files(dir: &str) -> anyhow::Result<String> {
	let mut patches: Vec<LibraryPatch> = Vec::new();
	let files = std::fs::read_dir(dir)
		.expect("could not read dir")
		.map(|file| file.expect("could not get file").path());

	for file_path in files {
		if let Some(extension) = file_path.extension() {
			if extension == "json" {
				let file_contents = std::fs::read_to_string(file_path).unwrap();
				let patch: LibraryPatch =
					serde_json::from_str(&file_contents).map_err(|err| return err).unwrap();
				patches.push(patch);
			}
		}
	}

	Ok(serde_json::to_string(&patches).unwrap())
}

pub fn uncollect_patch_files(dir: &str, file: &str) -> anyhow::Result<()> {
	let output = std::path::Path::new(dir);
	let data = std::fs::read_to_string(file).expect("failed to read file");
	let patches: Vec<LibraryPatch> = serde_json::from_str(&data).expect("failed to parse json");

	let mut file_names: HashMap<String, usize> = HashMap::new();
	let mut result: Vec<(String, LibraryPatch)> = Vec::new();

	for patch in patches.iter() {
		let spliced = patch
			.match_
			.first()
			.expect("could not get match name")
			.split(':')
			.collect::<Vec<&str>>()[1];
		let tmp_file_name = format!("{}.json", spliced);
		let unique_file_name = generate_unique_file_name(&mut file_names, tmp_file_name);

		file_names.insert(unique_file_name.clone(), 1);
		result.push((unique_file_name, patch.clone()));
	}

	for (file_name, patch) in &result {
		let writable = serde_json::to_string(patch).expect("could not parse json to string");

		std::fs::write(output.join(&file_name), writable).expect("failed to write file");
	}

	Ok(())
}

fn generate_unique_file_name(file_names: &mut HashMap<String, usize>, base_name: String) -> String {
	let mut count = 1;
	let mut file_name = base_name.clone();

	while file_names.contains_key(&file_name) {
		count += 1;
		file_name = format!("{}-{}.json", base_name, count);
	}

	file_name
}
