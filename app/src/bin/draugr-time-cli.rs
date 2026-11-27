#[path = "../cli.rs"]
mod cli;
fn main() {
	match cli::run(std::env::args().skip(1).collect()) {
		Ok(output) => println!("{output}"),
		Err(error) => {
			eprintln!("{error}");
			std::process::exit(1);
		}
	}
}
