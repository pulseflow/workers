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
	/// Merge JSON Patch files together
	MergeFiles(DirectoryArgs),
	/// Unmerge JSON Patch files
	UnmergeFiles(DirectoryArgs),
}

#[derive(Args)]
pub struct DirectoryArgs {
	/// The directory containing the patch files
	pub dir: String,
	/// The destination directory
	pub dest: String,
}

#[derive(Args)]
pub struct NoArgs {}
