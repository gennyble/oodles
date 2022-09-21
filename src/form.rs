use std::str::FromStr;

use hyper::StatusCode;
use mavourings::query::Query;

use crate::Request;

pub struct Login {
	pub username: String,
	pub password: String,
}

impl Login {
	pub async fn from_request(req: Request) -> Result<Self, StatusCode> {
		let query = QueryWrapper::from_post_body(req).await?;
		let username = query.get_first_value("username")?;
		let password = query.get_first_value("password")?;

		Ok(Self { username, password })
	}
}

pub struct OodleCreate {
	pub title: String,
	pub filename: String,
	pub content: String,
}

impl OodleCreate {
	pub async fn from_request(req: Request) -> Result<Self, StatusCode> {
		let query = QueryWrapper::from_post_body(req).await?;
		let title = query.get_first_value("title")?;
		let filename = query.get_first_value("filename")?;
		let content = query.get_first_value("firstPost")?;

		Ok(Self {
			title,
			filename,
			content,
		})
	}
}

pub struct MessageCreate {
	pub filename: String,
	pub content: String,
}

impl MessageCreate {
	pub async fn from_request(req: Request) -> Result<Self, StatusCode> {
		let query = QueryWrapper::from_post_body(req).await?;
		let filename = query.get_first_value("filename")?;
		let content = query.get_first_value("firstPost")?;

		Ok(Self { filename, content })
	}
}

pub struct MessageModify {
	pub filename: String,
	pub message_id: usize,
	pub content: String,
}

impl MessageModify {
	pub async fn from_request(req: Request) -> Result<Self, StatusCode> {
		let query = QueryWrapper::from_post_body(req).await?;
		let filename = query.get_first_value("filename")?;
		let message_id = query.parse_first_value("id")?;
		let content = query.get_first_value("content")?;

		Ok(Self {
			filename,
			message_id,
			content,
		})
	}
}

pub struct QueryWrapper(Query);

impl QueryWrapper {
	pub fn from_uri_query(req: &Request) -> Result<Self, StatusCode> {
		match req.query() {
			Some(Ok(q)) => Ok(QueryWrapper(q)),
			_ => return Err(StatusCode::BAD_REQUEST),
		}
	}

	pub async fn from_post_body(req: Request) -> Result<Self, StatusCode> {
		req.body_query()
			.await
			.map_err(|_| StatusCode::BAD_REQUEST)
			.map(|q| q.into())
	}

	pub fn get_first_value<S: AsRef<str>>(&self, key: S) -> Result<String, StatusCode> {
		self.0
			.get_first_value(key)
			.ok_or(StatusCode::BAD_REQUEST)
			.map(<_>::to_owned)
	}

	pub fn parse_first_value<T: FromStr, S: AsRef<str>>(&self, key: S) -> Result<T, StatusCode> {
		self.0
			.parse_first_value(key)
			.ok_or(StatusCode::BAD_REQUEST)?
			.map_err(|_| StatusCode::BAD_REQUEST)
	}
}

impl From<Query> for QueryWrapper {
	fn from(q: Query) -> Self {
		Self(q)
	}
}
