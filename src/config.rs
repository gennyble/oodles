use std::{
	net::{IpAddr, Ipv4Addr},
	path::PathBuf,
};

use confindent::Confindent;
use getopts::Options;

pub struct Config {
	pub address: IpAddr,
	pub port: u16,
	pub credential_file: PathBuf,
	pub data_directory: PathBuf,
}

impl Config {
	/// Parses the options and flags present on the command line.
	///
	/// # Returns
	/// An alternative path to the config file, or None if it was not specified, and this struct
	/// with the data filled in from flags.
	pub fn get() -> Self {
		let args: Vec<String> = std::env::args().collect();

		// Please stop wrapping the long calls to optopt. The vertical formatting is hard to read.
		// Is there a way to turn off just fn_call_width? #[rustfmt::skip(fn_call_width)] doesn't
		// seem to work. - gen
		#[rustfmt::skip]
		let opts = {
			let mut opts = Options::new();
			opts.optflag("h", "help", "Print this message and exit");
			opts.optopt("c", "config", "Alternate config file\nDefault: /etc/oodles/oodles.conf", "FILE");
			opts.optopt("p", "port", "The port to run the server on\nConfig Key: Port\nDefault: TODO", "PORT");
			opts.optopt("a", "address", "What IP address to serve on\nConfig Key: Address\nDefault: 127.0.0.1", "IPADDR");
			opts.optopt("", "credentials", "File to find login information\nConfig Key: CredentialFile", "FILE");
			opts.optopt("d", "data-directory", "Where data is to be kept\nConfig Key: DataDirectory", "PATH");
			opts
		};

		let usage = || print!("{}", opts.usage(&format!("Usage: {} [options]", args[0])));

		let matches = match opts.parse(&args[1..]) {
			Ok(m) => m,
			Err(_e) => todo!(),
		};

		if matches.opt_present("help") {
			usage();
			std::process::exit(0);
		}

		let conf_location: PathBuf = matches
			.opt_get("config")
			.expect("config option is not a path")
			.unwrap_or(PathBuf::from("/etc/oodles/oodles.conf"));

		let conf = Confindent::from_file(conf_location).expect("Failed to parse config");

		let cli_or_conf = |opt: &str, key: &str| -> Option<String> {
			matches
				.opt_str(opt)
				.or_else(|| conf.child_value(key).map(String::from))
		};

		let address: IpAddr = cli_or_conf("address", "Address")
			.map(|s| s.parse().expect("Failed to parse Address"))
			.unwrap_or(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));

		let port: u16 = cli_or_conf("port", "Port")
			.map(|s| s.parse().expect("Failed to parse Port"))
			.unwrap_or_else(|| todo!());

		let credential_file: PathBuf = cli_or_conf("credentials", "CredentialFile")
			.map(|s| s.parse().expect("Failed to parse CredentailFile path"))
			.unwrap_or(PathBuf::from("/etc/oodles/oodles.conf"));

		let data_directory: PathBuf = cli_or_conf("data-directory", "DataDirectory")
			.map(|s| s.parse().expect("Failed to parse DataDirectory path"))
			.expect("No Data Directory specified");

		Self {
			address,
			port,
			credential_file,
			data_directory,
		}
	}
}
