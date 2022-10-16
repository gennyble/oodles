use std::{
	cell::Ref,
	error::Error,
	fmt,
	path::{Path, PathBuf},
	str::FromStr,
};

use serde::{ser::SerializeStruct, Serialize};
use time::{format_description::FormatItem, macros::format_description, OffsetDateTime, UtcOffset};
use tokio::{fs::File, io::AsyncWriteExt};

#[derive(Clone, Debug, PartialEq)]
pub struct Backlink {
	pub oodle_id: String,
	pub message_id: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Oodle {
	pub id: String,
	pub name: String,
	pub file: PathBuf,
	pub messages: Vec<Message>,
	pub backlinks: Vec<Backlink>,
}

impl Oodle {
	pub fn new<I: Into<String>, N: Into<String>, P: Into<PathBuf>>(
		id: I,
		name: N,
		file: P,
		first_message: Message,
	) -> Self {
		Self {
			id: id.into(),
			name: name.into(),
			file: file.into(),
			messages: vec![first_message],
			backlinks: vec![],
		}
	}

	pub fn new_noid<N: Into<String>, P: Into<PathBuf>>(
		name: N,
		file: P,
		first_message: Message,
	) -> Self {
		Self {
			id: mavourings::users::random_base58(6),
			name: name.into(),
			file: file.into(),
			messages: vec![first_message],
			backlinks: vec![],
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

	fn extract_id(s: &str) -> Option<String> {
		if let Some(s) = s.strip_prefix("[") {
			if let Some(title) = s.strip_suffix("]") {
				return Some(title.to_owned());
			}
		}

		None
	}
}

impl fmt::Display for Oodle {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "-= {} =-\n", self.name)?;
		write!(f, "[{}]\n", self.id)?;

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
		let title = match s.find("\n") {
			Some(idx) => match Self::extract_title(&s[..idx]) {
				Some(title) => {
					s = &s[idx + 1..];
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

		let id = match s.find("\n") {
			Some(idx) => match Self::extract_id(&s[..idx]) {
				Some(title) => {
					s = &s[idx + 1..];
					Some(title)
				}
				None => {
					//TODO:gen- title malformed
					todo!()
				}
			},
			None => None,
		};

		if s.chars().next() == Some('\n') {
			// Skip it
			s = &s[1..];
		}

		let mut oodles = Self {
			id: id.unwrap_or(mavourings::users::random_base58(6)),
			name: title,
			file: PathBuf::from("/tmp"),
			messages: vec![],
			backlinks: vec![],
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
	pub references: Vec<Reference>,
	pub backlinks: Vec<Backlink>,
}

impl Message {
	const TIME_FORMAT: &'static[FormatItem<'static>] = format_description!("[year padding:zero repr:full base:calendar sign:automatic]-[month padding:zero repr:numerical]-[day padding:zero] [hour padding:zero repr:24]:[minute padding:zero]:[second padding:zero][offset_hour padding:zero sign:mandatory][offset_minute padding:zero]");

	pub fn new<M: Into<String>>(id: usize, date: OffsetDateTime, message: M) -> Self {
		let msg = message.into();
		Self {
			id,
			date,
			content: msg.clone(),
			references: Self::find_references(&msg),
			backlinks: vec![],
		}
	}

	//TODO: resolve references
	pub fn new_now<M: Into<String>>(message: M, offset: UtcOffset) -> Self {
		let msg = message.into();
		Self {
			id: 0,
			date: OffsetDateTime::now_utc().to_offset(offset),
			content: msg.clone(),
			references: Self::find_references(&msg),
			backlinks: vec![],
		}
	}

	fn find_references(mut s: &str) -> Vec<Reference> {
		let mut ret = vec![];

		loop {
			match s.find('{') {
				None => return ret,
				Some(start_idx) => match s.find('}') {
					None => return ret,
					Some(end_idx) => {
						let raw = &s[start_idx..end_idx + 1];
						s = &s[end_idx + 1..];
						let reference = Reference::from_str(raw);

						if let Ok(r) = reference {
							ret.push(r);
						}
					}
				},
			}
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

		Ok(Self::new(idx.unwrap_or(0), date, content.trim()))
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

#[derive(Clone, Debug, PartialEq)]
pub enum Reference {
	Message { oodle_id: String, message_id: usize },
	Oodle { oodle_id: String },
	Internal { message_id: usize },
}

impl fmt::Display for Reference {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Reference::Message {
				oodle_id,
				message_id,
			} => write!(f, "{{{oodle_id}/{message_id}}}"),
			Reference::Oodle { oodle_id } => write!(f, "{{{oodle_id}}}"),
			Reference::Internal { message_id } => write!(f, "{{~{message_id}}}"),
		}
	}
}

// This wants clean refernces. Like "{abcdef/4}" or "{~3}" not "also, in {~3}" or just "~3"
impl FromStr for Reference {
	type Err = Box<dyn Error>;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		if let Some(internal_with_hanging) = s.strip_prefix("{~") {
			// Internal message reference
			let internal = &internal_with_hanging[..internal_with_hanging.len() - 1];
			let message_id: usize = internal.parse()?;
			return Ok(Self::Internal { message_id });
		} else {
			let full = &s[1..s.len() - 1];

			match full.split_once('/') {
				None => Ok(Self::Oodle {
					oodle_id: full.into(),
				}),
				Some((oid, mid)) => {
					let message_id = mid.parse()?;

					Ok(Self::Message {
						oodle_id: oid.into(),
						message_id,
					})
				}
			}
		}
	}
}

#[cfg(test)]
mod test {
	use std::str::FromStr;

	use time::macros::{datetime, offset};

	use crate::{Message, Oodle, Reference};

	#[test]
	fn message_formats_correctly() {
		let message = Message::new(
			0,
			datetime!(2022-06-01 13:45 -5),
			"Line one!\nLine tw- oh no is that a\n.\nIt was!",
		);

		let expected =
			"2022-06-01 13:45:00-0500\nLine one!\nLine tw- oh no is that a\n..\nIt was!\n";

		assert_eq!(format!("{}", message), expected)
	}

	#[test]
	fn message_parses_correctly() {
		let message = Message::new(
			0,
			datetime!(2022-06-01 13:45 -5),
			"Line one!\nLine tw- oh no is that a\n.\nIt was!",
		);

		let expected = "2022-06-01 13:45:00-0500\nLine one!\nLine tw- oh no is that a\n..\nIt was!";

		assert_eq!(Message::from_str(expected), Ok(message))
	}

	#[test]
	fn oodle_formats_correctly() {
		let message = Message::new(
			0,
			datetime!(2022-06-01 13:45 -5),
			"Line one!\nLine tw- oh no is that a\n.\nIt was!",
		);

		let message2 = Message::new(
			1,
			datetime!(2022-06-01 14:15 -5),
			"Looky here another message!",
		);

		let expected =
			"-= Hey, I'm a title! =-\n[123456]\n\n2022-06-01 13:45:00-0500\nLine one!\nLine tw- oh no is that a\n..\nIt was!\n.\n\n2022-06-01 14:15:00-0500\nLooky here another message!\n.\n";

		let mut ood = Oodle::new("123456", "Hey, I'm a title!", "/tmp/nothing.oodle", message);
		ood.push_message(message2);

		assert_eq!(format!("{}", ood), expected)
	}

	#[test]
	fn oodle_format_index_jump_correctly() {
		let message = Message::new(
			0,
			datetime!(2022-06-01 13:45 -5),
			"Line one!\nLine tw- oh no is that a\n.\nIt was!",
		);

		let message2 = Message::new(
			2,
			datetime!(2022-06-01 14:15 -5),
			"Looky here another message!",
		);

		let expected =
			"-= Hey, I'm a title! =-\n[abcdef]\n\n2022-06-01 13:45:00-0500\nLine one!\nLine tw- oh no is that a\n..\nIt was!\n.\n\n2022-06-01 14:15:00-0500 (2)\nLooky here another message!\n.\n";

		let mut ood = Oodle::new("abcdef", "Hey, I'm a title!", "/tmp/nothing.oodle", message);
		ood.push_message(message2);

		assert_eq!(format!("{}", ood), expected)
	}

	#[test]
	fn oodle_parses_correctly() {
		let message = Message::new(
			0,
			datetime!(2022-06-01 13:45 -5),
			"Line one!\nLine tw- oh no is that a\n.\nIt was!",
		);

		let message2 = Message::new(
			1,
			datetime!(2022-06-01 14:15 -5),
			"Looky here another message!",
		);

		let expected =
			"-= Hey, I'm a title! =-\n[ABC123]\n\n2022-06-01 13:45:00-0500\nLine one!\nLine tw- oh no is that a\n..\nIt was!\n.\n\n2022-06-01 14:15:00-0500\nLooky here another message!\n.\n";

		let mut ood = Oodle::new("ABC123", "Hey, I'm a title!", "/tmp", message);
		ood.push_message(message2);

		assert_eq!(Oodle::from_str(expected), Ok(ood))
	}

	#[test]
	fn reference_writes_correctly() {
		let r = Reference::Internal { message_id: 0 };
		assert_eq!(r.to_string().as_str(), "{~0}");

		let r = Reference::Message {
			oodle_id: String::from("abcID"),
			message_id: 0,
		};
		assert_eq!(r.to_string().as_str(), "{abcID/0}");

		let r = Reference::Oodle {
			oodle_id: String::from("abcdefg"),
		};
		assert_eq!(r.to_string().as_str(), "{abcdefg}");
	}

	#[test]
	fn reference_parses_correctly() {
		let rp = "{~0}".parse();
		let r = Reference::Internal { message_id: 0 };
		assert_eq!(r, rp.unwrap());

		let rp = "{abcID/0}".parse();
		let r = Reference::Message {
			oodle_id: String::from("abcID"),
			message_id: 0,
		};
		assert_eq!(r, rp.unwrap());

		let rp = "{abcdefg}".parse();
		let r = Reference::Oodle {
			oodle_id: String::from("abcdefg"),
		};
		assert_eq!(r, rp.unwrap());
	}

	#[test]
	fn message_finds_references() {
		let msg = Message::new_now("Blh blah!\n{~2}", offset!(-5));

		assert_eq!(msg.references, vec![Reference::Internal { message_id: 2 }]);
	}
}
