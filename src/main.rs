use notify_rust::{Hint, Notification};
use reqwest;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio;

const VERSION: &str = "0.1.0";

struct Check<'a> {
	url: &'a str,
	name: &'a str,
}
const URLS: [Check; 3] = [
	Check {
		url: "https://ddr0.ca/gallery.html",
		name: "Gallery",
	},
	Check {
		url: "https://ddr0.ca/⚂/",
		name: "Roller",
	},
	Check {
		url: "https://ddr0.ca/⚂/ws/socket.io.js",
		name: "Roller Backend",
	},
];

struct Query<'a> {
	check: &'a Check<'a>,
	date: Duration, //Since unix_epoch, use as_secs() to get unix time.
	duration: Duration,
	status: i32,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let client = reqwest::Client::builder()
		.https_only(true)
		.redirect(reqwest::redirect::Policy::limited(0))
		.timeout(Duration::from_secs(30)) //max 999 for now, output is overwritten in place in columnar format
		.pool_idle_timeout(Duration::from_secs(5))
		.user_agent(format!("ddr0-watcher v{}", VERSION))
		//.http2_prior_knowledge() //Doesn't connect, even though http/2 is supported.
		.build()?;

	let mut queries = Vec::with_capacity(URLS.len());
	for check in URLS.iter() {
		let &Check { url, name: _ } = check;
		let start = Instant::now();
		let res = client.get(url).send().await;
		//println!("res in {}ms: {:?}", duration.as_millis(), res);

		let query = Query {
			check,
			date: SystemTime::now()
				.duration_since(UNIX_EPOCH)
				.expect("System time is currently before unix epoch."),
			duration: start.elapsed(),
			status: match res {
				Ok(resp) => resp.status().as_u16().into(),
				Err(e) => match e.status() {
					Some(status) => status.as_u16().into(),
					None => -1, //Timeout, construction problem, etc.
				},
			},
		};

		log(&query);
		alert(&query);
		queries.push(query);

		//tokio::time::sleep(Duration::new(1, 0)).await;
	}

	Ok(())
}

fn log(query: &Query) {
	println!(
		"{:>11.11}, {:>3.3}, {:>6.6}, {:<50.50}",
		query.date.as_secs(),
		query.status,
		query.duration.as_millis(),
		query.check.url,
	);
}

enum AlertAction {
	None,
	Warn,
	Error,
}
fn alert(query: &Query) -> AlertAction {
	let mut warning_level = AlertAction::None;
	const SLOW_THRESHOLD: Duration = Duration::from_millis(500);
	if query.status == 200 && query.duration > SLOW_THRESHOLD {
		Notification::new()
			.summary(&format!("DDR0.ca Slow {}", query.check.name).to_owned())
			.body(
				&format!(
					"{} > {} for {}.",
					query.duration.as_millis(),
					SLOW_THRESHOLD.as_millis(),
					query.check.url
				)
				.to_owned(),
			)
			.icon(&"warning".to_owned()) //dialog-warning?
			.show()
			.expect("Could not show notification.");
		warning_level = AlertAction::Warn;
	}

	match query.status {
		200 => warning_level,
		201..=299 => {
			Notification::new()
				.summary(
					&format!(
						"DDR0.ca {} Unexpected HTTP {}",
						query.check.name, query.status
					)
					.to_owned(),
				)
				.body(
					&format!(
						"{} returned HTTP {}, not HTTP 200 OK as expected.",
						query.check.url, query.status
					)
					.to_owned(),
				)
				.icon(&"warning".to_owned().to_owned()) //dialog-warning?
				.show()
				.expect("Could not show notification.");
			AlertAction::Warn
		}
		-1 => {
			//Not necessarily, but probably, a network error.
			Notification::new()
				.summary(&format!("DDR0.ca {} Down", query.check.name).to_owned())
				.body(
					&format!(
						"The HTTP request to {} could not be completed.",
						query.check.url
					)
					.to_owned(),
				)
				.icon(&"error".to_owned())
				.hint(Hint::Resident(true)) // this is not supported by all implementations
				.timeout(0) // this however is
				.show()
				.expect("Could not show notification.");
			AlertAction::Error
		}
		_ => {
			Notification::new()
				.summary(&format!("DDR0.ca {} Down", query.check.name).to_owned())
				.body(&format!("{} returned HTTP {}.", query.check.url, query.status).to_owned())
				.icon(&"error".to_owned())
				.hint(Hint::Resident(true)) // this is not supported by all implementations
				.timeout(0) // this however is
				.show()
				.expect("Could not show notification.");
			AlertAction::Error
		}
	}
}
