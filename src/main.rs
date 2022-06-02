use std::{
	future::Future,
	net::SocketAddr,
	pin::Pin,
	sync::Arc,
	task::{Context, Poll},
};

use hyper::{service::Service, Body, Request, Response, Server};
use oodles::Oodle;

mod config;

#[tokio::main]
async fn main() {
	let config = config::Config::get();

	println!(
		"Starting with Config:\n\t{}:{}\n\tCreds: {}\n\tData: {}",
		config.address,
		config.port,
		config.credential_file.to_string_lossy(),
		config.data_directory.to_string_lossy()
	);

	let database = Arc::new(Database {});

	let server = Server::bind(&SocketAddr::new(config.address, config.port)).serve(MakeSvc {
		database: database.clone(),
	});

	server.await.unwrap();
}

#[derive(Clone, Debug)]
struct Database {}

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
		todo!()
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

*/
