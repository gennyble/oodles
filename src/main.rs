use std::{
	future::Future,
	net::SocketAddr,
	pin::Pin,
	sync::Arc,
	task::{Context, Poll},
};

use argon2::{password_hash::SaltString, Argon2, PasswordHasher};
use database::Session;
use hyper::{header, service::Service, Body, Method, Request, Response, Server};
use mavourings::{
	file_reply, file_string_reply,
	query::{self, Query},
	template::Template,
};
use oodles::Message;
use rand::rngs::OsRng;
use time::{
	format_description::FormatItem,
	macros::{format_description, offset},
};

use crate::database::Database;

mod config;
mod database;

const DATETIME_FORMAT: &[FormatItem] = format_description!(
	"[weekday repr:long], [day padding:none] [year repr:full] [hour repr:24]:[minute padding:zero]"
);

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

	let database = Arc::new(Database::get(config.credential_file, config.data_directory));
	database.create_directories().await;
	database.oodles_mut().await.load_oodles().await;

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
			.trim_start_matches("/")
			.to_owned();

		let session = db.get_session(&req).await;

		if req.method() == Method::GET {
			if let Some(name) = path.strip_prefix("oodles/") {
				let name = query::Query::url_decode(name, false).unwrap();
				return Self::oodle_view(req, db, name, session).await;
			}
		}

		match (req.method(), path.as_str()) {
			(&Method::GET, "") | (&Method::GET, "index.html") => {
				Self::index(req, db, session).await
			}
			(&Method::GET, "style.css") => file_string_reply("web/style.css").await.unwrap(),
			(&Method::GET, "oodle.js") => file_string_reply("web/oodle.js").await.unwrap(),

			(&Method::GET, "logo.png") => file_reply("web/logo.png").await.unwrap(),
			(&Method::GET, "logo.svg") => file_string_reply("web/logo.svg").await.unwrap(),

			(&Method::GET, "login") => {
				if session.is_some() {
					Response::builder()
						.header("Location", "/")
						.status(302)
						.body(Body::default())
						.unwrap()
				} else {
					Template::file("web/login.html")
						.await
						.as_response()
						.unwrap()
				}
			}

			(&Method::POST, "login") => Self::user_login(req, db).await,
			(&Method::GET, "logout") => Self::user_logout(req, db, session).await,

			(&Method::POST, "oodle/create") => Self::oodle_create(req, db, session).await,
			(&Method::POST, "oodle/message/create") => Self::oodle_message(req, db, session).await,
			(&Method::POST, "oodle/message/modify") => {
				Self::oodle_message_modify(req, db, session).await
			}
			(&Method::GET, "oodle/message/get") => Self::oodle_message_get(req, db, session).await,

			_ => Response::builder().body(Body::from("404")).unwrap(),
		}
	}

	async fn index(
		mut _req: Request<Body>,
		db: Arc<Database>,
		session: Option<Session>,
	) -> Response<Body> {
		let mut tpl = Template::file("web/index.html").await;

		if let Some(sesh) = session {
			tpl.set("username", sesh.username);
		}

		for (title, datetime) in db.oodles().await.oodle_metedata().await {
			//TODO: gen- display dates, too
			let mut pattern = tpl.document.get_pattern("oodle").unwrap();
			pattern.set("name", title);
			pattern.set(
				"date",
				datetime
					.map(|odt| odt.format(DATETIME_FORMAT).unwrap())
					.unwrap(),
			);

			tpl.document.set_pattern("oodle", pattern);
		}

		tpl.as_response().unwrap()
	}

	async fn oodle_create(
		mut req: Request<Body>,
		db: Arc<Database>,
		session: Option<Session>,
	) -> Response<Body> {
		//FIXME: gen- check user has rights to make an oodle
		let body = hyper::body::to_bytes(req.body_mut()).await.unwrap();
		let body_string = String::from_utf8_lossy(&body);
		println!("{}", body_string);
		let query: Query = body_string.parse().unwrap();

		let title = query.get_first_value("title").unwrap();
		let filename = query.get_first_value("filename").unwrap();
		let content = query.get_first_value("firstPost").unwrap();

		//TODO: gen- Assocaite offset with user account.
		let message = Message::new_now(content, offset!(-5));
		db.oodles_mut()
			.await
			.new_oodle(title, filename, message)
			.await;

		Response::builder()
			.status(200)
			.header(header::LOCATION, "/")
			.status(302)
			.body(Body::from("Login success! Redirecting to home."))
			.unwrap()
	}

	async fn oodle_view(
		mut req: Request<Body>,
		db: Arc<Database>,
		name: String,
		session: Option<Session>,
	) -> Response<Body> {
		println!("Reqested oodle: {}", name);

		let oodles = db.oodles().await;
		let oodle = oodles.get_oodle_by_name(name).unwrap();

		let mut tpl = Template::file("web/oodle.html").await;
		tpl.set("name", oodle.name.clone());

		if let Some(sesh) = session {
			tpl.set("username", sesh.username);
			tpl.set(
				"filename",
				oodle.file.file_name().unwrap().to_string_lossy(),
			);
		}

		for msg in oodle.messages.iter() {
			let mut pattern = tpl.document.get_pattern("message").unwrap();

			//TODO: gen- actually format the date
			pattern.set("date", msg.date.format(DATETIME_FORMAT).unwrap());
			pattern.set("message", msg.content.replace("\n", "<br>"));
			pattern.set("message_id", format!("{}", msg.id));

			tpl.document.set_pattern("message", pattern);
		}

		tpl.as_response().unwrap()
	}

	async fn oodle_message(
		mut req: Request<Body>,
		db: Arc<Database>,
		session: Option<Session>,
	) -> Response<Body> {
		//TODO: gen- check the user actually has permission to add a message!
		let body = hyper::body::to_bytes(req.body_mut()).await.unwrap();
		let query: Query = String::from_utf8_lossy(&body).parse().unwrap();

		let message = query.get_first_value("content").unwrap();
		let filename = query.get_first_value("filename").unwrap();

		let name = {
			let mut oodles = db.oodles_mut().await;
			let oodle = oodles.oodle_by_file_mut(filename).unwrap();

			oodle.push_message(Message::new_now(message, offset!(-5)));
			oodle.save().await.unwrap();
			oodle.name.clone()
		};

		Response::builder()
			.status(200)
			.header(header::LOCATION, format!("/oodles/{}", name))
			.status(302)
			.body(Body::from("Oodle updated! Redirecting back to page"))
			.unwrap()
	}

	async fn oodle_message_get(
		mut req: Request<Body>,
		db: Arc<Database>,
		session: Option<Session>,
	) -> Response<Body> {
		//TODO: gen- check the user actually has permission to get this message!
		let body = req.uri().query().unwrap();
		let query: Query = body.parse().unwrap();

		let filename = query.get_first_value("filename").unwrap();
		let message_id: usize = query.parse_first_value("id").unwrap().unwrap();

		let oodles = db.oodles().await;
		let oodle = oodles.oodle_by_file(filename).unwrap();
		let message = oodle.message(message_id).unwrap();

		Response::builder()
			.status(200)
			.header("content-type", "application/json")
			.body(Body::from(serde_json::to_string(message).unwrap()))
			.unwrap()
	}

	async fn oodle_message_modify(
		mut req: Request<Body>,
		db: Arc<Database>,
		session: Option<Session>,
	) -> Response<Body> {
		//TODO: gen- check the user actually has permission to add a message!
		let body = hyper::body::to_bytes(req.body_mut()).await.unwrap();
		let query: Query = String::from_utf8_lossy(&body).parse().unwrap();

		let content = query.get_first_value("content").unwrap();
		let filename = query.get_first_value("filename").unwrap();
		let message_id: usize = query.parse_first_value("id").unwrap().unwrap();

		let name = {
			let mut oodles = db.oodles_mut().await;
			let oodle = oodles.oodle_by_file_mut(filename).unwrap();
			oodle.message_mut(message_id).unwrap().content = content.to_owned();
			oodle.save().await.unwrap();

			oodle.name.clone()
		};

		Response::builder()
			.status(200)
			.header(header::LOCATION, format!("/oodles/{}", name))
			.status(302)
			.body(Body::from("Oodle updated! Redirecting back to page"))
			.unwrap()
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
