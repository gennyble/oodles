use std::{
	collections::HashMap,
	error::Error,
	future::Future,
	net::SocketAddr,
	path::Path,
	pin::Pin,
	sync::Arc,
	task::{Context, Poll},
	time::Duration,
};

use argon2::{password_hash::SaltString, Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use hyper::{header, service::Service, Body, Method, Request, Response, Server};
use mavourings::{file_string_reply, query::Query, template::Template};
use oodles::Oodle;
use rand::{rngs::OsRng, Rng};
use tokio::sync::RwLock;

mod config;

#[tokio::main]
async fn main() {
	let command = std::env::args().nth(1);

	if let Some("encrypt") = command.as_deref() {
		command_encrypt()
	}

	let config = config::Config::get();

	println!(
		"Starting with Config:\n\t{}:{}\n\tCreds: {}\n\tData: {}",
		config.address,
		config.port,
		config.credential_file.to_string_lossy(),
		config.data_directory.to_string_lossy()
	);

	let database = Arc::new(Database::get(config.credential_file));

	let server = Server::bind(&SocketAddr::new(config.address, config.port)).serve(MakeSvc {
		database: database.clone(),
	});

	server.await.unwrap();
}

fn command_encrypt() -> ! {
	loop {
		let mut line = String::new();
		std::io::stdin().read_line(&mut line).unwrap();

		let salt = SaltString::generate(&mut OsRng);
		let argon2 = Argon2::default();
		let hash = argon2
			.hash_password(line.trim().as_bytes(), &salt)
			.unwrap()
			.to_string();

		println!("{}", hash);
	}
}

#[derive(Debug)]
struct Database {
	users: RwLock<Users>,
}

impl Database {
	pub fn get<C: AsRef<Path>>(credentials: C) -> Self {
		Database {
			users: RwLock::new(Users::load_file(credentials)),
		}
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
struct Users {
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
struct Session {
	cookie: String,
	username: String,
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

struct MakeSvc {
	database: Arc<Database>,
}

impl<T> Service<T> for MakeSvc {
	type Response = Svc;
	type Error = &'static str;
	type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

	fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
		Poll::Ready(Ok(()))
	}

	fn call(&mut self, _: T) -> Self::Future {
		let database = self.database.clone();
		let fut = async move { Ok(Svc { database }) };
		Box::pin(fut)
	}
}

struct Svc {
	database: Arc<Database>,
}

impl Service<Request<Body>> for Svc {
	type Response = Response<Body>;
	type Error = &'static str;
	type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

	fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
		Poll::Ready(Ok(()))
	}

	fn call(&mut self, req: Request<Body>) -> Self::Future {
		let db = self.database.clone();
		Box::pin(async { Ok(Self::task(req, db).await) })
	}
}

impl Svc {
	async fn task(req: Request<Body>, db: Arc<Database>) -> Response<Body> {
		let path = req
			.uri()
			.path()
			.trim_end_matches("/")
			.trim_start_matches("/");

		let session = db.get_session(&req).await;

		match (req.method(), path) {
			(&Method::GET, "") | (&Method::GET, "index.html") => {
				Self::index(req, db, session.as_ref()).await
			}
			(&Method::GET, "login") => {
				if session.is_some() {
					Response::builder()
						.header("Location", "/")
						.status(302)
						.body(Body::default())
						.unwrap()
				} else {
					file_string_reply("web/login.html").await.unwrap()
				}
			}
			(&Method::GET, "style.css") => file_string_reply("web/style.css").await.unwrap(),

			(&Method::POST, "login") => Self::user_login(req, db).await,
			(&Method::GET, "logout") => Self::user_logout(req, db, session).await,
			_ => Response::builder().body(Body::from("404")).unwrap(),
		}
	}

	async fn index(
		mut req: Request<Body>,
		db: Arc<Database>,
		session: Option<&Session>,
	) -> Response<Body> {
		let user_value = session
			.map(|s| format!("{} <a href='/logout'>(logout)</a>", s.username))
			.unwrap_or(String::from("<a href='/login'>login</a>"));

		let mut tpl = Template::file("web/index.html").await;

		//FIXME: gen- we want bempline to have and `else` for `if-set` so that
		// the username can remain unset and I don't have to do this
		if session.is_some() {
			tpl.set("postpermission", "true")
		}

		tpl.set("username", user_value);
		tpl.as_response().unwrap()
	}

	async fn user_login(mut req: Request<Body>, db: Arc<Database>) -> Response<Body> {
		let body = hyper::body::to_bytes(req.body_mut()).await.unwrap();
		let query: Query = String::from_utf8_lossy(&body).parse().unwrap();

		let username = query.get_first_value("username").unwrap();
		let password = query.get_first_value("password").unwrap();

		let builder = Response::builder().status(200);

		if db.verify_user_login(username, password).await {
			let session = db.new_user_session(username).await;

			builder
				.header(header::SET_COOKIE, session.get_set_cookie())
				.header(header::LOCATION, "/")
				.status(302)
				.body(Body::from("Login success! Redirecting to home."))
		} else {
			builder.body(Body::from("INVALID username or password"))
		}
		.unwrap()
	}

	async fn user_logout(
		_req: Request<Body>,
		db: Arc<Database>,
		session: Option<Session>,
	) -> Response<Body> {
		if session.is_none() {
			return Response::builder()
				.status(500)
				.body(Body::from("No session cookie set! How did you get here?"))
				.unwrap();
		}

		let session = session.unwrap();
		let clear = session.get_clear_cookie();
		db.delete_session(session.cookie).await;

		Response::builder()
			.status(302)
			.header(header::SET_COOKIE, clear)
			.header(header::LOCATION, "/")
			.status(302)
			.body(Body::from("Logged out, redirecting home."))
			.unwrap()
	}
}

/*
TODO:
- get server accepting connections
- send back static pages
- small login flow

Document generation:
We think this is the main driver for the design of the services and things so
we're going to think about it a little here.

"Files first" is one of our philosophy of software design. The files should,
when it makes sense, be able to be read by a human as well as the software.

I use linebreaks a lot when I type, so I need to allow them in whatever kind of
format I go with. That rules out line-per-message files with newlines escaped
as \n because that isn't very readable. Perhaps something like this:

```
20220-6-01 20:03 -500
This is a test message. The date and time indicate the start of a message and
a period of it's own line followed by an empty line. If the user wants a
lone period line (LPL), we should change that to a double period while saving
and convert back while loading. Like what SMTP does.
.
```

We like that format! A lot, actually. *gentle humming*
Don't wrap the lines, either~

What's next? We don't want to generate the document on every request, that seems
silly. We should update the storage file, described above, and then use it to
generate static html afterwards which we read from the FS on GET.

Ah, Oodle's (new canon for the name of the format) have to save the title
somewhere. Let's stick it at the beginning of the file and have it present
as important. Probably one of:

-= Title =-
~ Title ~
# Title
Title
/!\ Title /!\

Okay okaaay so we chose -= Title =- and also we remembered that timezones
existed. So we can't just send a static file back. There has to be *some*
dynamic filling if we don't want a script running on the client. So we can
generate a bempline document and fill in the times when the client requests.
Perhaps we can have a little thing to set a cookie with the timezone so it's
easier. A little warning near the top of the page if we don't have their
timezone cookie set yet.

Dangit we did the todo out of order. Got sidetracked by messages and oodles.
Time for the servery bits and accounts.

2022-08-07 18:03 -800
It seems I did accounts and things last time! Nice, thanks me. But I forgot
the password for testuser and it's hashed, so. I made it `password` because that
seems alright.
.

Okay, it's tomorrow. We want to be able to hide messages. We should worry about
that later and just get the prototype working. But if we don't write it down now
it'll be trapped in this stupid head.

So we have the OffsetDateTime on a line by itself and this leads in the Message.
What I want to do is to be able to add metadata on a message. Mostly I just want
to indicate if it's hidden or not, but I would also *love* to be able to save
and edit time, too. I think the edit time can come later and be on the next
line. Something like `edited <offsetdatetime>` and then we just look for that.
It keeps with the "readable and writeable to humans" theme going, see. So I want
to keep that with the meta, too. I just want to be able to mark messages as
hidden, see? I think I could do that by leading the OffsetDateTime with an
asterisk, but we can worry later.
*/
