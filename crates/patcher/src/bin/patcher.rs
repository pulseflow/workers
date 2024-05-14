use clap::Parser;
use meta_patcher::cli::{Cli, Commands};
use meta_patcher::patch::{collect_patch_files, uncollect_patch_files};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let cli = Cli::parse();

	match &cli.command {
		Commands::MergeFiles(args) => {
			let merged = collect_patch_files(&args.dir).unwrap();
			std::fs::write(&args.dest, merged)?;
		}
		Commands::UnmergeFiles(args) => {
			uncollect_patch_files(&args.dest, &args.dir)?;
		}
	}

	Ok(())
}
