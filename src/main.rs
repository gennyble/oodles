mod config;

fn main() {
	let config = config::Config::get();

	println!(
		"Starting with Config:\n\t{}:{}\n\tCreds: {}\n\tData: {}",
		config.address,
		config.port,
		config.credential_file.to_string_lossy(),
		config.data_directory.to_string_lossy()
	);
}
