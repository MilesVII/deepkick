fn main() {
	// Link the VC runtime statically on Windows/MSVC so `deepkick.exe` runs on a clean
	// machine without the Visual C++ redistributable (the UCRT ships with Windows 10+).
	static_vcruntime::metabuild();

	// Embed the app icon and version info into the Windows executable.
	if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
		println!("cargo:rerun-if-changed=deepkick.ico");
		let mut resource = winresource::WindowsResource::new();
		resource
			.set_icon("deepkick.ico")
			.set("ProductName", "DeepKick Lyrics")
			.set("FileDescription", "DeepKick Lyrics console utility");
		if let Err(e) = resource.compile() {
			println!("cargo:warning=could not embed the Windows icon: {e}");
		}
	}
}
