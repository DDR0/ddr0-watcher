use notify_rust::{Hint, Notification};
use reqwest;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio;

const VERSION: &str = "0.2.0";

struct Job<'a> {
	project: &'a str,
	redirects: usize,
	checks: Vec<Check<'a>>,
}
struct Check<'a> {
	url: &'a str,
	component: &'a str,
}

struct Query<'a> {
	check: &'a Check<'a>,
	job: &'a Job<'a>,
	date: Duration, //Since unix_epoch, use as_secs() to get unix time.
	duration: Duration,
	status: i32,
}

#[derive(PartialEq)]
enum AlertAction {
	None,
	Warn,
	Error,
}

fn jobs() -> Vec<Job<'static>> {
	vec![
		Job {
			project: "ddr0.ca",
			redirects: 0,
			checks: vec![
				Check {
					component: "Gallery",
					url: "https://ddr0.ca/gallery.html",
				},
				Check {
					component: "Roller",
					url: "https://ddr0.ca/⚂/",
				},
				Check {
					component: "Roller Backend",
					url: "https://ddr0.ca/⚂/ws/socket.io.js",
				},
			],
		},
		Job {
			project: "ravelights.ca",
			redirects: 2,
			checks: vec![
				Check {
					component: "Landing Page",
					url: "https://ravelights.ca/",
				},
				Check {
					component: "Redirect",
					url: "https://flaketechnologies.ca/",
				},
			],
		},
	]
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let jobs = jobs();
	let mut handles = Vec::with_capacity(jobs.len());
	for job in jobs.iter() {
		handles.push(run(job))
	}
	
	futures::future::join_all(handles).await;
	
	Ok(())
}

async fn run(job: &Job<'_>) -> Result<(), Box<dyn std::error::Error>> {
	let client = reqwest::Client::builder()
		.https_only(true)
		.redirect(reqwest::redirect::Policy::limited(job.redirects))
		.timeout(Duration::from_secs(2)) //max 999 for now, output is overwritten in place in columnar format
		.pool_idle_timeout(Duration::from_secs(1))
		.user_agent(format!("DDR's Watcher {}", VERSION))
		//.http2_prior_knowledge() //Doesn't connect, even though http/2 is supported.
		.build()?;
	
	let mut queries = Vec::with_capacity(job.checks.len());
	for check in job.checks.iter() {
		let &Check { url, component: _ } = check;
		let start = Instant::now();
		let res = client.get(url).send().await;
		
		let query = Query {
			job,
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
		if alert(&query) == AlertAction::Error {
			break;
		}
		queries.push(query);

		tokio::time::sleep(Duration::new(1, 0)).await;
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

fn alert(query: &Query) -> AlertAction {
	let mut warning_level = AlertAction::None;
	const SLOW_THRESHOLD: Duration = Duration::from_millis(500);
	if query.status == 200 && query.duration > SLOW_THRESHOLD {
		Notification::new()
			.summary(&format!("{} Slow {}", query.job.project, query.check.component).to_owned())
			.body(&format!("{} > {} for {}.", query.duration.as_millis(), SLOW_THRESHOLD.as_millis(), query.check.url).to_owned())
			.icon(&"dialog-warning".to_owned()) //dialog-warning?
			.show()
			.expect("Could not show notification.");
		warning_level = AlertAction::Warn;
	}

	match query.status {
		200 => warning_level,
		201..=299 => {
			Notification::new()
				.summary(&format!("{} {} Unexpected HTTP {}", query.job.project, query.check.component, query.status).to_owned())
				.body(&format!("{} returned HTTP {}, not HTTP 200 OK as expected.", query.check.url, query.status).to_owned())
				.icon(&"dialog-warning".to_owned().to_owned()) //dialog-warning?
				.show()
				.expect("Could not show notification.");
			AlertAction::Warn
		}
		-1 => {
			//Not necessarily, but probably, a network error.
			Notification::new()
				.summary(&format!("{} {} Down", query.job.project, query.check.component).to_owned())
				.body(&format!("The HTTP request to {} could not be completed.", query.check.url).to_owned())
				.icon(&"dialog-error".to_owned())
				.hint(Hint::Resident(true)) // this is not supported by all implementations
				.timeout(0) // this however is
				.show()
				.expect("Could not show notification.");
			AlertAction::Error
		}
		_ => {
			Notification::new()
				.summary(&format!("{} {} Down", query.job.project, query.check.component).to_owned())
				.body(&format!("{} returned HTTP {}.", query.check.url, query.status).to_owned())
				.icon(&"dialog-error".to_owned())
				.hint(Hint::Resident(true)) // this is not supported by all implementations
				.timeout(0) // this however is
				.show()
				.expect("Could not show notification.");
			AlertAction::Error
		}
	}
}
