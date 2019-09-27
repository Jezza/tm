use std::path::Path;

use chrono::{Duration, Datelike, DateTime, FixedOffset, Local, Timelike, Weekday};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
struct Args {
	#[structopt(subcommand)]
	command: Command
}

#[derive(Debug, StructOpt)]
enum Command {
	Start {
		message: Vec<String>,
	},
	Stop {
		message: Vec<String>,
	},
	Cycle,
	Check {
		args: Vec<String>,
	},
}

fn main() {
	let data = {
		let mut data_dir = dirs::data_dir()
			.unwrap();

		data_dir.push("tm");
		data_dir.push("tm.dat");

		data_dir
	};

	let Args {
		command
	} = <_>::from_args();

	match command {
		Command::Start {
			message
		} => append_to_file(&data, "start", &message),
		Command::Stop {
			message
		} => append_to_file(&data, "stop", &message),
		Command::Cycle => cycle_data(&data),
		Command::Check {
			args
		} => check_data(&data, &args),
	}
}

fn append_to_file(data: &Path, action: &'static str, message: &Vec<String>) {
	use std::fs::create_dir_all;
	use std::io::Write;

	let now = Local::now().to_rfc3339();

	let message = message.join(" ");

	let line = format!("{}|{}|{}\n", now, action, message);

	create_dir_all(data.parent().unwrap()).unwrap();

	use std::fs::OpenOptions;
	let mut file = OpenOptions::new()
		.create(true)
		.append(true)
		.open(data)
		.unwrap();

	file.write_all(line.as_bytes()).unwrap();
}

fn cycle_data(data: &Path) {
	if !data.exists() {
		println!("Nothing to cycle.");
		return;
	}

	use std::fs::rename;

	let now = Local::now().to_rfc3339();

	let target = data.parent()
		.unwrap()
		.join(format!("tm.dat.{}", now));

	rename(&data, &target).unwrap();
}

fn check_data(data: &Path, search_spec: &Vec<String>) {
	let window = build_time_window(search_spec);

	let marks = collect_marks(data, window);

	let mut buffer = String::new();

	let columns = 24 * 8;

	const DAY_WIDTH: usize = 11;

	{
		buffer.push('┌');
		buffer.push_str(&format!("{}", "─".repeat(DAY_WIDTH)));
		buffer.push('┬');
		buffer.push_str(&"─".repeat(columns));
		buffer.push('┐');
//		buffer.push('\n');

//		┌ ┐ └ ┘ ─ │ ┬ ┴ ┼ ├ ┤
	}

	{
		// 0 to 192
		// 1 to 191
		// 0 to 190
		// Every 8 minutes, new character...

		// Day header
		let mut current_day: Option<DateTime<FixedOffset>> = None;

		const SKIP_FILL: u32 = 192u32;
		let mut fill_character = " ";

		// Body
		let mut current_time = SKIP_FILL;

		for mark in marks {
			{
				let start_of_day = mark.start_of_day();

				let new_day = if let Some(current) = current_day {
					if current < start_of_day {
						if (current + Duration::days(1)) != start_of_day {
							let remaining = SKIP_FILL - current_time;
							if remaining > 0 {
								buffer.push_str(&format!("{}", fill_character.repeat(remaining as usize)));
								buffer.push('│');
							}
							buffer.push_str("\n├");
							buffer.push_str(&format!("{}", "─".repeat(DAY_WIDTH)));
							buffer.push('┼');
							buffer.push_str(&format!("{}", "─".repeat(columns)));
							buffer.push_str("┤");
							current_time = SKIP_FILL;
						}
						true
					} else if current > start_of_day {
						panic!("Times are out of order. {} > {}", current, start_of_day);
					} else {
						false
					}
				} else {
					true
				};

				if new_day {
					let remaining = SKIP_FILL - current_time;
					if remaining > 0 {
						buffer.push_str(&format!("{}", fill_character.repeat(remaining as usize)));
						buffer.push('│');
					}

					buffer.push_str("\n│");
					let name = weekday_name(start_of_day.weekday());
					buffer.push_str(&format!("{:^11}", name));
					buffer.push('│');
					current_day = Some(start_of_day);
					current_time = 0;
				}
			}

			let (time, character) = match mark {
				Mark::Start {
					time,
					..
				} => (time, " "),
				Mark::Stop {
					time,
					..
				} => (time, "="),
				Mark::InferredStart {
					end: time,
					..
				} => (time, "S"),
				Mark::InferredStop {
					end: time,
					..
				} => (time, "E"),
			};

			fill_character = match mark {
				Mark::Start {
					time,
					..
				} => "=",
				Mark::Stop {
					time,
					..
				} => " ",
				Mark::InferredStart {
					end: time,
					..
				} => "S",
				Mark::InferredStop {
					end: time,
					..
				} => "E",
			};

			let characters = time.num_seconds_from_midnight() / 60 / 8;
			let width = characters - current_time;

			buffer.push_str(&character.repeat(width as usize));

			current_time = characters;
		}

		let remaining = SKIP_FILL - current_time;
		if remaining > 0 {
			buffer.push_str(&format!("{}", fill_character.repeat(remaining as usize)));
			buffer.push('│');
		}
	}


	{
		buffer.push_str("\n");
		buffer.push('└');
		buffer.push_str(&format!("{}", "─".repeat(DAY_WIDTH)));
		buffer.push('┴');
		buffer.push_str(&"─".repeat(columns));
		buffer.push('┘');
		buffer.push('\n');
	}

	print!("{}", buffer);

//	println!("====================================");
//	for mark in &marks {
//		println!("{:?}", mark);
//	}
//	println!("====================================");
}

fn weekday_name(day: Weekday) -> &'static str {
	match day {
		Weekday::Mon => "Monday",
		Weekday::Tue => "Tuesday",
		Weekday::Wed => "Wednesday",
		Weekday::Thu => "Thursday",
		Weekday::Fri => "Friday",
		Weekday::Sat => "Saturday",
		Weekday::Sun => "Sunday",
	}
}

fn collect_marks(data: &Path, window: Window<DateTime<FixedOffset>>) -> Vec<Mark> {
	let mut marks = vec![];

	macro_rules! add {
	    ($($item:expr),*) => {
			$(
	    		{
					let mark: Mark = $item;
					if mark.is_within(&window) {
						marks.push(mark);
					}
	    		}
	    	)*
	    };
	}


	let mut last: Option<Mark> = None;

	let content = std::fs::read_to_string(data).unwrap();
	for line in content.lines() {
//		println!("line = {:?}", line);
		if line.is_empty() {
			continue;
		}

		let mut t = line.split("|");
		let time = DateTime::parse_from_rfc3339(t.next().unwrap()).unwrap();
//		println!("time = {:?}", time);
		let action = t.next().unwrap();
//		println!("action = {:?}", action);
		let message = t.collect::<Vec<_>>().join("|");
//		println!("message = {:?}", message);

		let mark = match &*action {
			"start" => Mark::Start {
				time,
				message,
			},
			"stop" => Mark::Stop {
				time,
				message,
			},
			_ => panic!("Unknown action: {}", action),
		};

//		println!("---------");
//		println!("Mark: {:?}", mark);

		if let Some(last) = last {
			match mark {
				Mark::Start {
					time: end,
					..
				} => if let Mark::Start {
					time: start,
					..
				} = last {
					add!(last, Mark::InferredStop {
						start,
						end,
					});
				} else {
					add!(last);
				},
				Mark::Stop {
					time: end,
					..
				} => if let Mark::Stop {
					time: start,
					..
				} = last {
					add!(last, Mark::InferredStart {
						start,
						end,
					});
				} else {
					add!(last);
				},
				m => {
					// This mark is only created base on the action,
					// so we can't get an inferred* here, as there's no action for inferred marks.
					panic!("Unknown mark type: {:?}", m)
				}
			}
		}

		last = Some(mark);
//		println!("=========");
	}
	if let Some(last) = last {
		add!(last);
	}

	marks
}

#[derive(Debug, PartialEq)]
enum Mark {
	Start {
		time: DateTime<FixedOffset>,
		message: String,
	},
	Stop {
		time: DateTime<FixedOffset>,
		message: String,
	},
	InferredStart {
		start: DateTime<FixedOffset>,
		end: DateTime<FixedOffset>,
	},
	InferredStop {
		start: DateTime<FixedOffset>,
		end: DateTime<FixedOffset>,
	},
}

impl Mark {
	fn is_within(&self, window: &Window<DateTime<FixedOffset>>) -> bool {
		match self {
			Mark::Start {
				time,
				..
			} => window.contains(time),
			Mark::Stop {
				time,
				..
			} => window.contains(time),
			Mark::InferredStart {
				start,
				end,
			} => window.contains(start) || window.contains(end),
			Mark::InferredStop {
				start,
				end,
			} => window.contains(start) || window.contains(end),
		}
	}

	fn start_of_day(&self) -> DateTime<FixedOffset> {
		self.start_bound()
			.with_hour(0)
			.unwrap()
			.with_minute(0)
			.unwrap()
			.with_second(0)
			.unwrap()
			.with_nanosecond(0)
			.unwrap()
	}

	fn start_bound(&self) -> &DateTime<FixedOffset> {
		match self {
			Mark::Start {
				time,
				..
			} => time,
			Mark::Stop {
				time,
				..
			} => time,
			Mark::InferredStart {
				start,
				..
			} => start,
			Mark::InferredStop {
				start,
				..
			} => start,
		}
	}

	fn end_bound(&self) -> &DateTime<FixedOffset> {
		match self {
			Mark::Start {
				time,
				..
			} => time,
			Mark::Stop {
				time,
				..
			} => time,
			Mark::InferredStart {
				end,
				..
			} => end,
			Mark::InferredStop {
				end,
				..
			} => end,
		}
	}
}

fn build_time_window(search_spec: &Vec<String>) -> Window<DateTime<FixedOffset>> {
//	println!("Args: {:?}", search_spec);
	if search_spec.is_empty() {
		return Window::Unbound;
	}

	// this [day] -> the last time that day happened.
	// this week
	// this month
	// last [day]
	// last week
	// last month

	let now: DateTime<FixedOffset> = Local::now().into();

	let duration = Duration::minutes(90);

	Window::new(now - duration, now)
}

enum Window<T> {
	Unbound,
	Bound {
		start: T,
		end: T,
	},
}

impl<T> Window<T> {
	fn new(start: T, end: T) -> Self {
		Window::Bound {
			start,
			end,
		}
	}

	fn contains<U>(&self, item: &U) -> bool
		where
			T: PartialOrd<U>,
			U: ?Sized + PartialOrd<T>,
	{
		match self {
			Window::Unbound => true,
			Window::Bound {
				start,
				end,
			} => {
				start < item && item < end
			}
		}
	}
}