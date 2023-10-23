use crate::utils::*;

use interpulse::{
	api::minecraft::{
		merge_partial_library,
		Library,
		PartialLibrary,
		VersionManifest,
	},
	utils::get_hash,
};
use meta_patcher::patch::LibraryPatch;

