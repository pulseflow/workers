use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
	#[command(subcommand)]
	pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
	/// merge a directory of unmerged patch filesinto a merged patch file
	MergeFiles(DirectoryArgs),
	/// unmerge a merged patch file into a directory contaning each patch file
	UnmergeFiles(DirectoryArgs),
}

#[derive(Args)]
pub struct DirectoryArgs {
	/// the directory containing the patch files
	pub dir: Option<String>,
	/// the directory or file containing the merged patch files
	pub dest: Option<String>,
}
