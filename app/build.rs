fn main() {
	println!("cargo:rerun-if-changed=../logo/draugr-time.ico");
	#[cfg(target_os = "windows")]
	winresource::WindowsResource::new()
		.set_icon("../logo/draugr-time.ico")
		.set("ProductName", "Draugr Time")
		.set("FileDescription", "Draugr Time")
		.compile()
		.expect("compile Draugr Time Windows icon resource");
}
