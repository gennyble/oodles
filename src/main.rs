use std::{
	future::Future,
	net::SocketAddr,
	pin::Pin,
	sync::Arc,
	task::{Context, Poll},
};

use argon2::{password_hash::SaltString, Argon2, PasswordHasher};
use database::Session;
use form::QueryWrapper;
use hyper::{header, service::Service, Body, Method, Response, Server, StatusCode};
use mavourings::{
	file_reply, file_string_reply,
	query::{self, Query, QueryParseError},
	template::Template,
};
use oodles::{Backlink, Message};
use rand::rngs::OsRng;
use serde::de::DeserializeOwned;
use time::{
	format_description::FormatItem,
	macros::{format_description, offset},
};

use crate::database::Database;

mod config;
mod database;
mod form;

const DATETIME_FORMAT: &[FormatItem] = format_description!(
	"[weekday repr:long], [month repr:long] [day padding:none] [year repr:full] [hour repr:24]:[minute padding:zero]"
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

	println!("Listening on http://{}:{}", config.address, config.port);

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

pub struct Request {
	pub inner: hyper::Request<Body>,
}

impl Request {
	pub fn method(&self) -> &Method {
		self.inner.method()
	}

	pub fn path(&self) -> &str {
		self.inner.uri().path()
	}

	pub fn query(&self) -> Option<Result<Query, QueryParseError>> {
		self.inner.uri().query().map(|q| q.parse())
	}

	pub async fn body_query(self) -> Result<Query, QueryParseError> {
		self.into_string_body().await.parse()
	}

	pub async fn into_string_body(mut self) -> String {
		let body = hyper::body::to_bytes(self.inner.body_mut()).await.unwrap();
		String::from_utf8_lossy(&body).into_owned()
	}

	pub async fn json<T: DeserializeOwned>(mut self) -> Result<T, serde_json::Error> {
		let body = hyper::body::to_bytes(self.inner.body_mut()).await.unwrap();
		serde_json::from_slice(&body)
	}
}

impl From<hyper::Request<Body>> for Request {
	fn from(inner: hyper::Request<Body>) -> Self {
		Self { inner }
	}
}

struct Svc {
	database: Arc<Database>,
}

impl Service<hyper::Request<Body>> for Svc {
	type Response = Response<Body>;
	type Error = &'static str;
	type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

	fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
		Poll::Ready(Ok(()))
	}

	fn call(&mut self, req: hyper::Request<Body>) -> Self::Future {
		let db = self.database.clone();
		Box::pin(async { Ok(Self::task(req.into(), db).await) })
	}
}

impl Svc {
	async fn task(req: Request, db: Arc<Database>) -> Response<Body> {
		let path = req
			.inner
			.uri()
			.path()
			.trim_end_matches("/")
			.trim_start_matches("/")
			.to_owned();

		let session = db.get_session(&req.inner).await;

		let response = match (req.method(), path.as_str()) {
			(&Method::GET, "") | (&Method::GET, "index.html") => {
				Ok(Self::index(req, db, session).await)
			}
			(&Method::GET, "style.css") => Ok(file_string_reply("web/style.css").await.unwrap()),
			(&Method::GET, "oodle.js") => Ok(file_string_reply("web/oodle.js").await.unwrap()),

			(&Method::GET, "logo.png") => Ok(file_reply("web/logo.png").await.unwrap()),
			(&Method::GET, "logo.svg") => Ok(file_string_reply("web/logo.svg").await.unwrap()),

			(&Method::GET, "login") => Ok(if session.is_some() {
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
			}),

			(&Method::POST, "login") => Self::user_login(req, db).await,
			(&Method::GET, "logout") => Self::user_logout(req, db, session).await,

			(&Method::POST, "oodle/create") => Self::oodle_create(req, db, session).await,
			(&Method::POST, "oodle/message/create") => Self::oodle_message(req, db, session).await,
			(&Method::POST, "oodle/message/modify") => {
				Self::oodle_message_modify(req, db, session).await
			}
			(&Method::GET, "oodle/message/get") => Self::oodle_message_get(req, db, session).await,

			(&Method::GET, _) => {
				if let Some(name) = path.strip_prefix("oodles/") {
					let name = query::Query::url_decode(name, false).unwrap();
					Self::oodle_view(req, db, name, session).await
				} else {
					Err(StatusCode::NOT_FOUND)
				}
			}

			_ => Err(StatusCode::NOT_FOUND),
		};

		match response {
			Ok(response) => response,
			Err(status) => Response::builder()
				.status(status)
				.body(Body::from(format!("{}", status.as_str())))
				.unwrap(),
		}
	}

	async fn index(
		mut _req: Request,
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
		req: Request,
		db: Arc<Database>,
		session: Option<Session>,
	) -> Result<Response<Body>, StatusCode> {
		//FIXME: gen- check user has rights to make an oodle
		let form = form::OodleCreate::from_request(req).await?;

		//TODO: gen- Assocaite offset with user account.
		let message = Message::new_now(form.content, offset!(-5));
		db.oodles_mut()
			.await
			.new_oodle(form.title, form.filename, message)
			.await;

		Ok(Response::builder()
			.status(200)
			.header(header::LOCATION, "/")
			.status(302)
			.body(Body::from("Login success! Redirecting to home."))
			.unwrap())
	}

	//TODO: gen- Error handling
	async fn oodle_view(
		req: Request,
		db: Arc<Database>,
		name: String,
		session: Option<Session>,
	) -> Result<Response<Body>, StatusCode> {
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

		for bl in &oodle.backlinks {
			let mut blt = tpl.get_pattern("oodle_backlinks").unwrap();
			blt.set("backlink", format!("{}/{}", bl.oodle_id, bl.message_id));
			tpl.set_pattern("oodle_backlinks", blt);
		}

		for msg in oodle.messages.iter() {
			let mut pattern = tpl.document.get_pattern("message").unwrap();

			//TODO: gen- actually format the date
			pattern.set("date", msg.date.format(DATETIME_FORMAT).unwrap());
			pattern.set("message", msg.content.replace("\n", "<br>"));
			pattern.set("message_id", format!("{}", msg.id));

			for abl in &msg.backlinks {
				let mut bl = pattern.get_pattern("backlink").unwrap();
				bl.set("backlink", format!("{}/{}", abl.oodle_id, abl.message_id));
				pattern.set_pattern("backlink", bl);
			}

			tpl.document.set_pattern("message", pattern);
		}

		Ok(tpl.as_response().unwrap())
	}

	async fn oodle_message(
		req: Request,
		db: Arc<Database>,
		session: Option<Session>,
	) -> Result<Response<Body>, StatusCode> {
		let query: Query = req.query().unwrap().unwrap();
		if query.has_bool("json") {
			let json: form::MessageCreate =
				req.json().await.map_err(|_| StatusCode::BAD_REQUEST)?;

			let tpl = {
				let mut oodles = db.oodles_mut().await;
				let oodle = oodles
					.oodle_by_file_mut(json.filename)
					.ok_or(StatusCode::NOT_FOUND)?;

				let message = Message::new_now(json.content, offset!(-5));

				let mut tpl = Template::file("web/oodle_message.html").await;

				if let Some(se) = session {
					tpl.set("username", se.username);
				}

				tpl.set("message", message.content.replace("\n", "<br>"));
				tpl.set("date", message.date.format(DATETIME_FORMAT).unwrap());

				let refs = message.references.clone();
				let oid = oodle.id.clone();
				let id = oodle.push_message(message);

				tpl.set("message_id", id);

				oodle
					.save()
					.await
					.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

				oodles.map_backlinks(
					Backlink {
						oodle_id: oid,
						message_id: id,
					},
					&refs,
				);

				tpl
			};

			tpl.as_response()
				.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
		} else {
			//TODO: gen- check the user actually has permission to add a message!
			let form = form::MessageCreate::from_request(req).await?;

			let name = {
				let mut oodles = db.oodles_mut().await;
				let oodle = oodles
					.oodle_by_file_mut(form.filename)
					.ok_or(StatusCode::NOT_FOUND)?;

				oodle.push_message(Message::new_now(form.content, offset!(-5)));
				oodle
					.save()
					.await
					.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
				oodle.name.clone()
			};

			Ok(Response::builder()
				.status(200)
				.header(header::LOCATION, format!("/oodles/{}", name))
				.status(302)
				.body(Body::from("Oodle updated! Redirecting back to page"))
				.unwrap())
		}
	}

	async fn oodle_message_get(
		req: Request,
		db: Arc<Database>,
		session: Option<Session>,
	) -> Result<Response<Body>, StatusCode> {
		//TODO: gen- check the user actually has permission to get this message!
		let query = QueryWrapper::from_uri_query(&req)?;
		let filename = query.get_first_value("filename")?;
		let message_id: usize = query.parse_first_value("id")?;

		let oodles = db.oodles().await;
		let oodle = oodles
			.oodle_by_file(filename)
			.ok_or(StatusCode::NOT_FOUND)?;
		let message = oodle.message(message_id).ok_or(StatusCode::NOT_FOUND)?;

		Ok(Response::builder()
			.status(200)
			.header("content-type", "application/json")
			.body(Body::from(serde_json::to_string(message).unwrap()))
			.unwrap())
	}

	async fn render_message(message: &Message) -> Template {
		let mut tpl = Template::file("web/oodle_message.html").await;

		tpl.set("message", message.content.replace("\n", "<br>"));
		tpl.set("date", message.date.format(DATETIME_FORMAT).unwrap());
		tpl.set("message_id", message.id);

		tpl
	}

	async fn oodle_message_modify(
		req: Request,
		db: Arc<Database>,
		session: Option<Session>,
	) -> Result<Response<Body>, StatusCode> {
		//TODO: gen- check the user actually has permission to add a message!
		let query: Query = req.query().unwrap().unwrap();
		if query.has_bool("json") {
			let json: form::MessageModify =
				req.json().await.map_err(|_| StatusCode::BAD_REQUEST)?;

			let mut tpl = {
				let mut oodles = db.oodles_mut().await;
				let oodle = oodles
					.oodle_by_file_mut(json.filename)
					.ok_or(StatusCode::NOT_FOUND)?;

				let tpl = {
					let msg = oodle.message_mut(json.id).ok_or(StatusCode::NOT_FOUND)?;
					msg.content = json.content;
					Self::render_message(msg).await
				};

				oodle
					.save()
					.await
					.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

				tpl
			};

			if let Some(se) = session {
				tpl.set("username", se.username);
			}

			tpl.as_response()
				.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
		} else {
			let form = form::MessageModify::from_request(req).await?;

			let name = {
				let mut oodles = db.oodles_mut().await;
				let oodle = oodles
					.oodle_by_file_mut(form.filename)
					.ok_or(StatusCode::NOT_FOUND)?;

				oodle
					.message_mut(form.id)
					.ok_or(StatusCode::NOT_FOUND)?
					.content = form.content;

				oodle
					.save()
					.await
					.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

				oodle.name.clone()
			};

			Ok(Response::builder()
				.status(200)
				.header(header::LOCATION, format!("/oodles/{}", name))
				.status(302)
				.body(Body::from("Oodle updated! Redirecting back to page"))
				.unwrap())
		}
	}

	async fn user_login(req: Request, db: Arc<Database>) -> Result<Response<Body>, StatusCode> {
		let form = form::Login::from_request(req).await?;

		let builder = Response::builder().status(200);

		Ok(
			if db.verify_user_login(&form.username, &form.password).await {
				let session = db.new_user_session(form.username).await;

				builder
					.header(header::SET_COOKIE, session.get_set_cookie())
					.header(header::LOCATION, "/")
					.status(302)
					.body(Body::from("Login success! Redirecting to home."))
			} else {
				builder.body(Body::from("INVALID username or password"))
			}
			.unwrap(),
		)
	}

	async fn user_logout(
		_req: Request,
		db: Arc<Database>,
		session: Option<Session>,
	) -> Result<Response<Body>, StatusCode> {
		if session.is_none() {
			return Err(StatusCode::BAD_REQUEST);
		}

		let session = session.unwrap();
		let clear = session.get_clear_cookie();
		db.delete_session(session.cookie).await;

		Ok(Response::builder()
			.status(302)
			.header(header::SET_COOKIE, clear)
			.header(header::LOCATION, "/")
			.status(302)
			.body(Body::from("Logged out, redirecting home."))
			.unwrap())
	}
}
