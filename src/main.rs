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

*/
