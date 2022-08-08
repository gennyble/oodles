use std::{
	fmt,
	path::{Path, PathBuf},
	str::FromStr,
};

use time::{format_description::FormatItem, macros::format_description, OffsetDateTime, UtcOffset};
use tokio::{fs::File, io::AsyncWriteExt};

#[derive(Clone, Debug, PartialEq)]
pub struct Oodle {
	pub name: String,
	pub file: PathBuf,
	pub messages: Vec<Message>,
}

impl Oodle {
	pub fn new<N: Into<String>, P: Into<PathBuf>>(
		name: N,
		file: P,
		first_message: Message,
	) -> Self {
		Self {
			name: name.into(),
			file: file.into(),
			messages: vec![first_message],
		}
	}

	pub fn push_message(&mut self, msg: Message) {
		self.messages.push(msg);
	}

	pub async fn save(&self) -> Result<(), std::io::Error> {
		let mut file = File::create(&self.file).await?;
		file.write(format!("{}", self).as_bytes()).await.map(|_| ())
	}

	pub async fn read<P: AsRef<Path>>(path: P) -> Result<Oodle, std::io::Error> {
		let mut oodle: Oodle = std::fs::read_to_string(path.as_ref())?.parse().unwrap();
		oodle.file = path.as_ref().to_owned();
		Ok(oodle)
	}

	pub fn date(&self) -> Option<OffsetDateTime> {
		self.messages.first().map(|m| m.date)
	}

	fn extract_title(s: &str) -> Option<String> {
		if let Some(s) = s.strip_prefix("-=") {
			if let Some(title) = s.strip_suffix("=-") {
				return Some(title.trim().to_owned());
			}
		}

		None
	}
}

impl fmt::Display for Oodle {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "-= {} =-\n", self.name)?;

		for msg in &self.messages {
			write!(f, "\n{}.\n", msg)?;
		}

		Ok(())
	}
}

impl FromStr for Oodle {
	//TODO: gen- real error
	type Err = ();

	fn from_str(mut s: &str) -> Result<Self, Self::Err> {
		let title = match s.find("\n\n") {
			Some(idx) => match Self::extract_title(&s[..idx]) {
				Some(title) => {
					s = &s[idx + 2..];
					title
				}
				None => {
					//TODO:gen- title malformed
					todo!()
				}
			},
			None => {
				//TODO:gen- No title was present.
				todo!()
			}
		};

		let mut messages: Vec<Message> = vec![];

		loop {
			match s.find("\n.\n") {
				Some(idx) => {
					messages.push(s[..idx].trim().parse()?);
					s = &s[idx + 3..];
				}
				None => break,
			}
		}

		if !s.trim().is_empty() {
			messages.push(s.trim().parse()?);
		}

		Ok(Self {
			name: title,
			file: PathBuf::from("/tmp"),
			messages,
		})
	}
}

#[derive(Clone, Debug, PartialEq)]
pub struct Message {
	pub date: OffsetDateTime,
	pub content: String,
}

impl Message {
	const TIME_FORMAT: &'static[FormatItem<'static>] = format_description!("[year padding:zero repr:full base:calendar sign:automatic]-[month padding:zero repr:numerical]-[day padding:zero] [hour padding:zero repr:24]:[minute padding:zero]:[second padding:zero][offset_hour padding:zero sign:mandatory][offset_minute padding:zero]");

	pub fn new_now<M: Into<String>>(message: M, offset: UtcOffset) -> Self {
		Self {
			date: OffsetDateTime::now_utc().to_offset(offset),
			content: message.into(),
		}
	}

	pub fn formatted_date(&self) -> String {
		self.date
			.format(Self::TIME_FORMAT)
			.expect("Failed to format date. Why?")
	}
}

impl fmt::Display for Message {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}\n", self.formatted_date())?;

		for line in self.content.lines() {
			if line == "." {
				write!(f, "..\n")?;
			} else {
				write!(f, "{}\n", line)?;
			}
		}

		Ok(())
	}
}

impl FromStr for Message {
	//TODO: gen- a more descriptive error
	type Err = ();

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let mut lines = s.lines();

		let date = if let Some(datetime) = lines.next() {
			//TODO: gen- return an error rather than panic
			match OffsetDateTime::parse(datetime, Self::TIME_FORMAT) {
				Ok(dt) => dt,
				Err(e) => panic!("{}", e),
			}
		} else {
			// No datetime present! *(/nothing/ present)
			return Err(());
		};

		let mut content = String::new();
		for line in lines {
			if line == ".." {
				content.push_str(".\n");
			} else {
				content.push_str(line);
				content.push('\n');
			}
		}

		Ok(Self {
			date,
			content: content.trim().to_owned(),
		})
	}
}

#[cfg(test)]
mod test {
	use std::str::FromStr;

	use time::macros::datetime;

	use crate::{Message, Oodle};

	#[test]
	fn message_formats_correctly() {
		let message = Message {
			date: datetime!(2022-06-01 13:45 -5),
			content: String::from("Line one!\nLine tw- oh no is that a\n.\nIt was!"),
		};

		let expected =
			"2022-06-01 13:45:00-0500\nLine one!\nLine tw- oh no is that a\n..\nIt was!\n";

		assert_eq!(format!("{}", message), expected)
	}

	#[test]
	fn message_parses_correctly() {
		let message = Message {
			date: datetime!(2022-06-01 13:45 -5),
			content: String::from("Line one!\nLine tw- oh no is that a\n.\nIt was!"),
		};

		let expected = "2022-06-01 13:45:00-0500\nLine one!\nLine tw- oh no is that a\n..\nIt was!";

		assert_eq!(Message::from_str(expected), Ok(message))
	}

	#[test]
	fn oodle_formats_correctly() {
		let message = Message {
			date: datetime!(2022-06-01 13:45 -5),
			content: String::from("Line one!\nLine tw- oh no is that a\n.\nIt was!"),
		};

		let message2 = Message {
			date: datetime!(2022-06-01 14:15 -5),
			content: String::from("Looky here another message!"),
		};

		let expected =
			"-= Hey, I'm a title! =-\n\n2022-06-01 13:45:00-0500\nLine one!\nLine tw- oh no is that a\n..\nIt was!\n.\n\n2022-06-01 14:15:00-0500\nLooky here another message!\n.\n";

		let mut ood = Oodle::new("Hey, I'm a title!", "/tmp/nothing.oodle", message);
		ood.push_message(message2);

		assert_eq!(format!("{}", ood), expected)
	}

	#[test]
	fn oodle_parses_correctly() {
		let message = Message {
			date: datetime!(2022-06-01 13:45 -5),
			content: String::from("Line one!\nLine tw- oh no is that a\n.\nIt was!"),
		};

		let message2 = Message {
			date: datetime!(2022-06-01 14:15 -5),
			content: String::from("Looky here another message!"),
		};

		let expected =
			"-= Hey, I'm a title! =-\n\n2022-06-01 13:45:00-0500\nLine one!\nLine tw- oh no is that a\n..\nIt was!\n.\n\n2022-06-01 14:15:00-0500\nLooky here another message!\n.\n";

		let mut ood = Oodle::new("Hey, I'm a title!", "/tmp", message);
		ood.push_message(message2);

		assert_eq!(Oodle::from_str(expected), Ok(ood))
	}
}
