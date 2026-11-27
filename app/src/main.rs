#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! Native Draugr Time desktop application.
mod branding;
#[cfg(not(target_os = "windows"))]
mod cli;
mod desktop;
mod icons;
fn main() -> iced::Result {
	#[cfg(not(target_os = "windows"))]
	{
		let args: Vec<String> = std::env::args().skip(1).collect();
		if args.first().is_some_and(|arg| {
			!arg.starts_with('-')
				|| [
					"--help",
					"-h",
					"--data-dir",
					"--from",
					"--to",
					"--json",
					"--format",
					"--group",
					"--tag",
				]
				.contains(&arg.as_str())
		}) {
			match cli::run(args) {
				Ok(output) => println!("{output}"),
				Err(error) => {
					eprintln!("{error}");
					std::process::exit(1);
				}
			}
			return Ok(());
		}
	}
	desktop::run()
}
