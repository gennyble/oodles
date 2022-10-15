use std::{
	collections::HashMap,
	path::{Path, PathBuf},
	time::Duration,
};

use argon2::{Argon2, PasswordHash, PasswordVerifier};
use hyper::{header, Request};
use oodles::{Message, Oodle};
use rand::{rngs::OsRng, Rng};
use time::OffsetDateTime;
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

#[derive(Debug)]
pub struct Oodles {
	oodle_directory: PathBuf,
	data: Vec<Oodle>,
}

impl Oodles {
	pub fn new<P: Into<PathBuf>>(data_dir: P) -> Self {
		let mut oodle_directory = data_dir.into();
		oodle_directory.push("oodles");

		Self {
			oodle_directory,
			data: vec![],
		}
	}

	pub async fn load_oodles(&mut self) {
		for entry in std::fs::read_dir(&self.oodle_directory).unwrap() {
			let oodle = Oodle::read(entry.unwrap().path()).await.unwrap();
			self.data.push(oodle);
		}
	}

	pub async fn new_oodle<T: Into<String>, F: Into<String>>(
		&mut self,
		title: T,
		filename: F,
		message: Message,
	) {
		let mut oodle_path = self.oodle_directory.clone();
		oodle_path.push(filename.into());

		let oodle = Oodle::new_noid(title, oodle_path, message);
		oodle.save().await.unwrap();

		self.data.push(oodle);
	}

	pub async fn oodle_metedata(&self) -> Vec<(String, Option<OffsetDateTime>)> {
		self.data
			.iter()
			.map(|oodle| (oodle.name.to_owned(), oodle.date()))
			.collect()
	}

	pub fn get_oodle_by_name<S: AsRef<str>>(&self, name: S) -> Option<&Oodle> {
		self.data
			.iter()
			.find(|&o| o.name.to_lowercase() == name.as_ref().to_lowercase())
	}

	pub fn oodle_by_file<P: Into<PathBuf>>(&self, file: P) -> Option<&Oodle> {
		let file = file.into();
		self.data
			.iter()
			.find(|o| o.file.file_name().unwrap() == file.file_name().unwrap())
	}

	pub fn oodle_by_file_mut<P: Into<PathBuf>>(&mut self, file: P) -> Option<&mut Oodle> {
		let file = file.into();
		self.data.iter_mut().find(|o| {
			println!("{}", o.file.file_name().unwrap().to_string_lossy());
			o.file.file_name().unwrap() == file.file_name().unwrap()
		})
	}
}

#[derive(Debug)]
pub struct Database {
	_data_directory: PathBuf,

	users: RwLock<Users>,
	oodles: RwLock<Oodles>,
}

impl Database {
	pub fn get<C: AsRef<Path>, D: Into<PathBuf>>(credentials: C, data_directory: D) -> Self {
		let data_directory = data_directory.into();
		Database {
			_data_directory: data_directory.clone(),

			users: RwLock::new(Users::load_file(credentials)),
			oodles: RwLock::new(Oodles::new(&data_directory)),
		}
	}

	pub async fn create_directories(&self) {
		if !self.oodles.read().await.oodle_directory.exists() {
			std::fs::create_dir(&self.oodles.read().await.oodle_directory).unwrap()
		}
	}

	pub async fn oodles(&self) -> RwLockReadGuard<Oodles> {
		self.oodles.read().await
	}

	pub async fn oodles_mut(&self) -> RwLockWriteGuard<Oodles> {
		self.oodles.write().await
	}

	pub async fn verify_user_login<U: AsRef<str>, P: AsRef<str>>(
		&self,
		username: U,
		password: P,
	) -> bool {
		let hash = {
			let lock = self.users.read().await;
			lock.users.get(username.as_ref()).map(String::to_owned)
		};

		if let Some(hash) = hash {
			let parsed_hash = PasswordHash::new(&hash).unwrap();

			Argon2::default()
				.verify_password(password.as_ref().as_bytes(), &parsed_hash)
				.is_ok()
		} else {
			false
		}
	}

	pub async fn new_user_session<U: AsRef<str>>(&self, username: U) -> Session {
		self.users.write().await.new_session(username).clone()
	}

	//TODO: gen- this is gross
	pub async fn get_session<T>(&self, req: &Request<T>) -> Option<Session> {
		if let Some(cook) = req.headers().get(header::COOKIE) {
			let cookie = mavourings::cookie::parse_header(cook.to_str().unwrap())
				.unwrap()
				.get("sid")
				.map(|s| (*s).to_owned());

			if let Some(cookie) = cookie {
				self.users
					.read()
					.await
					.get_session(cookie)
					.map(Session::to_owned)
			} else {
				None
			}
		} else {
			None
		}
	}

	pub async fn delete_session(&self, sid: String) -> bool {
		self.users.write().await.delete_session(sid)
	}
}

#[derive(Clone, Debug)]
pub struct Users {
	users: HashMap<String, String>,
	sessions: Vec<Session>,
}

impl Users {
	const BASE58: &'static [u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
	const SESSION_ID_LENGTH: usize = 6;

	pub fn load_file<C: AsRef<Path>>(credentials: C) -> Users {
		let string = std::fs::read_to_string(credentials).unwrap();

		let mut users = HashMap::new();
		for line in string.lines() {
			match line.split_once(" ") {
				Some((username, hash)) => {
					users.insert(username.into(), hash.into());
				}
				_ => panic!("did not understand credential file format"),
			}
		}

		Users {
			users,
			sessions: vec![],
		}
	}

	pub fn new_session<U: AsRef<str>>(&mut self, username: U) -> &Session {
		let cookie = Self::random_base58(Self::SESSION_ID_LENGTH);

		let session = Session::new(cookie, username.as_ref().into());
		self.sessions.push(session);
		self.sessions.last().unwrap()
	}

	pub fn get_session(&self, sid: String) -> Option<&Session> {
		self.sessions.iter().find(|&s| s.cookie == sid)
	}

	pub fn delete_session(&mut self, sid: String) -> bool {
		let op = self
			.sessions
			.iter()
			.enumerate()
			.find(|(_, sesh)| sesh.cookie == sid);
		if let Some((idx, _)) = op {
			self.sessions.swap_remove(idx);
			true
		} else {
			false
		}
	}

	fn random_base58(count: usize) -> String {
		let mut ret = String::with_capacity(count);

		let mut rng = OsRng::default();
		for _ in 0..count {
			let ridx = rng.gen_range(0..Self::BASE58.len());
			ret.push(Self::BASE58[ridx] as char)
		}

		ret
	}
}

#[derive(Clone, Debug)]
pub struct Session {
	pub cookie: String,
	pub username: String,
}

impl Session {
	pub fn new(cookie: String, username: String) -> Self {
		Self { cookie, username }
	}

	pub fn get_set_cookie(&self) -> String {
		mavourings::cookie::SetCookie::new("sid".into(), self.cookie.clone())
			.secure(true)
			.httponly(true)
			.max_age(Some(Duration::from_secs(60 * 60 * 24 * 7)))
			.as_string()
	}

	pub fn get_clear_cookie(&self) -> String {
		mavourings::cookie::SetCookie::new("sid".into(), self.cookie.clone())
			.secure(true)
			.httponly(true)
			.max_age(Some(Duration::from_secs(0)))
			.as_string()
	}
}
