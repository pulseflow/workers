use clap::Parser;
use meta_patcher::cli::{Cli, Commands};
use meta_patcher::patch::{collect_patch_files, uncollect_patch_files};

fn main() -> eyre::Result<()> {
	color_eyre::install()?;
	let cli = Cli::parse();

	match &cli.command {
		Commands::Merge(args) => collect_patch_files(&args.dir, &args.dest)?,
		Commands::Unmerge(args) => uncollect_patch_files(&args.dir, &args.dest)?,
	}

	Ok(())
}
