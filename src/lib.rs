use std::{
	fmt,
	path::{Path, PathBuf},
	str::FromStr,
};

use serde::{ser::SerializeStruct, Serialize};
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

	pub fn push_message(&mut self, mut msg: Message) -> usize {
		let idx = self.messages.last().map(|m| m.id + 1).unwrap_or(0);

		if msg.id > 0 {
			// Message declared it's own index
			if msg.id < idx {
				// but our index is bigger?? ignore the message index.
				msg.id = idx;
			}
		} else {
			// they were the first index or were not declared. either way we can set to 0
			msg.id = idx;
		}

		let id = msg.id;
		self.messages.push(msg);

		id
	}

	pub fn message(&self, index: usize) -> Option<&Message> {
		self.messages.iter().find(|msg| msg.id == index)
	}

	pub fn message_mut(&mut self, index: usize) -> Option<&mut Message> {
		self.messages.iter_mut().find(|msg| msg.id == index)
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

		let mut idx = 0;
		for msg in &self.messages {
			// Weird indexes are fixed on write, so we don't have to check low/high here.
			write!(f, "\n")?;
			if idx != msg.id {
				idx = msg.id;
				msg.fmt_with_idx(f)?;
			} else {
				write!(f, "{}", msg)?;
			}
			write!(f, ".\n")?;

			idx += 1;
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

		let mut oodles = Self {
			name: title,
			file: PathBuf::from("/tmp"),
			messages: vec![],
		};

		loop {
			match s.find("\n.\n") {
				Some(string_idx) => {
					let message: Message = s[..string_idx].trim().parse()?;
					oodles.push_message(message);
					s = &s[string_idx + 3..];
				}
				None => break,
			}
		}

		if !s.trim().is_empty() {
			oodles.push_message(s.trim().parse()?);
		}

		Ok(oodles)
	}
}

#[derive(Clone, Debug, PartialEq)]
pub struct Message {
	pub id: usize,
	pub date: OffsetDateTime,
	pub content: String,
}

impl Message {
	const TIME_FORMAT: &'static[FormatItem<'static>] = format_description!("[year padding:zero repr:full base:calendar sign:automatic]-[month padding:zero repr:numerical]-[day padding:zero] [hour padding:zero repr:24]:[minute padding:zero]:[second padding:zero][offset_hour padding:zero sign:mandatory][offset_minute padding:zero]");

	pub fn new_now<M: Into<String>>(message: M, offset: UtcOffset) -> Self {
		Self {
			id: 0,
			date: OffsetDateTime::now_utc().to_offset(offset),
			content: message.into(),
		}
	}

	pub fn formatted_date(&self) -> String {
		self.date
			.format(Self::TIME_FORMAT)
			.expect("Failed to format date. Why?")
	}

	pub fn fmt_with_idx(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.fmt_write_dateline(f, true)?;
		self.fmt_write_body(f)
	}

	fn fmt_write_dateline(&self, f: &mut fmt::Formatter<'_>, print_index: bool) -> fmt::Result {
		write!(f, "{}", self.formatted_date())?;

		if print_index {
			write!(f, " ({})", self.id)?;
		}

		write!(f, "\n")
	}

	fn fmt_write_body(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		for line in self.content.lines() {
			if line == "." {
				write!(f, "..\n")?;
			} else {
				write!(f, "{}\n", line)?;
			}
		}

		Ok(())
	}

	fn parse_dateline(line: &str) -> Result<(Option<usize>, OffsetDateTime), ()> {
		let (idx, dateline) = if line.ends_with(')') {
			match line.rsplit_once(" ") {
				Some((date, idx)) => {
					let idx = &idx[1..idx.len() - 1];
					let idx = usize::from_str_radix(idx, 10).unwrap();
					(Some(idx), date)
				}
				None => panic!("malformed dateline"),
			}
		} else {
			(None, line)
		};

		//TODO: gen- return an error rather than panic
		match OffsetDateTime::parse(dateline, Self::TIME_FORMAT) {
			Ok(dt) => return Ok((idx, dt)),
			Err(e) => panic!("{}", e),
		}
	}
}

impl fmt::Display for Message {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.fmt_write_dateline(f, false)?;
		self.fmt_write_body(f)
	}
}

impl FromStr for Message {
	//TODO: gen- a more descriptive error
	type Err = ();

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let mut lines = s.lines();

		let (idx, date) = if let Some(dateline) = lines.next() {
			Self::parse_dateline(dateline)?
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
			id: idx.unwrap_or(0),
			date,
			content: content.trim().to_owned(),
		})
	}
}

impl Serialize for Message {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer,
	{
		let epoch = self.date - OffsetDateTime::UNIX_EPOCH;

		let mut state = serializer.serialize_struct("Message", 3)?;
		state.serialize_field("id", &self.id)?;
		state.serialize_field("date", &epoch.whole_seconds())?;
		state.serialize_field("content", &self.content)?;
		state.end()
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
			id: 0,
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
			id: 0,
			date: datetime!(2022-06-01 13:45 -5),
			content: String::from("Line one!\nLine tw- oh no is that a\n.\nIt was!"),
		};

		let expected = "2022-06-01 13:45:00-0500\nLine one!\nLine tw- oh no is that a\n..\nIt was!";

		assert_eq!(Message::from_str(expected), Ok(message))
	}

	#[test]
	fn oodle_formats_correctly() {
		let message = Message {
			id: 0,
			date: datetime!(2022-06-01 13:45 -5),
			content: String::from("Line one!\nLine tw- oh no is that a\n.\nIt was!"),
		};

		let message2 = Message {
			id: 1,
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
	fn oodle_format_index_jump_correctly() {
		let message = Message {
			id: 0,
			date: datetime!(2022-06-01 13:45 -5),
			content: String::from("Line one!\nLine tw- oh no is that a\n.\nIt was!"),
		};

		let message2 = Message {
			id: 2,
			date: datetime!(2022-06-01 14:15 -5),
			content: String::from("Looky here another message!"),
		};

		let expected =
			"-= Hey, I'm a title! =-\n\n2022-06-01 13:45:00-0500\nLine one!\nLine tw- oh no is that a\n..\nIt was!\n.\n\n2022-06-01 14:15:00-0500 (2)\nLooky here another message!\n.\n";

		let mut ood = Oodle::new("Hey, I'm a title!", "/tmp/nothing.oodle", message);
		ood.push_message(message2);

		assert_eq!(format!("{}", ood), expected)
	}

	#[test]
	fn oodle_parses_correctly() {
		let message = Message {
			id: 0,
			date: datetime!(2022-06-01 13:45 -5),
			content: String::from("Line one!\nLine tw- oh no is that a\n.\nIt was!"),
		};

		let message2 = Message {
			id: 1,
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
