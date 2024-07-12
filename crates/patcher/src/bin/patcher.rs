use clap::Parser;
use meta_patcher::cli::{Cli, Commands};
use meta_patcher::patch::{collect_patch_files, uncollect_patch_files};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let cli = Cli::parse();

	match &cli.command {
		Commands::MergeFiles(args) => collect_patch_files(&args.dir, &args.dest)?,
		Commands::UnmergeFiles(args) => uncollect_patch_files(&args.dest, &args.dir)?,
	}

	Ok(())
}
