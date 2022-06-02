use std::{fmt, path::PathBuf};

use time::{format_description::FormatItem, macros::format_description, OffsetDateTime, UtcOffset};
use tokio::{fs::File, io::AsyncWriteExt};

pub struct Oodle {
	name: String,
	file: PathBuf,
	messages: Vec<Message>,
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
}

impl fmt::Display for Oodle {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "-= {} =-\n", self.name)?;

		for msg in &self.messages {
			write!(f, "\n{}", msg)?;
		}

		Ok(())
	}
}

pub struct Message {
	pub date: OffsetDateTime,
	pub content: String,
}

impl Message {
	const TIME_FORMAT: &'static[FormatItem<'static>] = format_description!("[year padding:zero repr:full base:calendar sign:automatic]-[month padding:zero repr:numerical]-[day padding:zero] [hour padding:zero repr:24]:[minute padding:zero]:[second padding:zero][offset_hour padding:none sign:mandatory][offset_minute padding:zero]");

	pub fn new_now(message: String, offset: UtcOffset) -> Self {
		Self {
			date: OffsetDateTime::now_utc().to_offset(offset),
			content: message,
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

		write!(f, ".\n")
	}
}

#[cfg(test)]
mod test {
	use time::macros::datetime;

	use crate::{Message, Oodle};

	#[test]
	fn message_formats_correctly() {
		let message = Message {
			date: datetime!(2022-06-01 13:45 -5),
			content: String::from("Line one!\nLine tw- oh no is that a\n.\nIt was!"),
		};

		let expected =
			"2022-06-01 13:45:00-500\nLine one!\nLine tw- oh no is that a\n..\nIt was!\n.\n";

		assert_eq!(format!("{}", message), expected)
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
			"-= Hey, I'm a title! =-\n\n2022-06-01 13:45:00-500\nLine one!\nLine tw- oh no is that a\n..\nIt was!\n.\n\n2022-06-01 14:15:00-500\nLooky here another message!\n.\n";

		let mut ood = Oodle::new("Hey, I'm a title!", "/tmp/nothing.oodle", message);
		ood.push_message(message2);

		assert_eq!(format!("{}", ood), expected)
	}
}
