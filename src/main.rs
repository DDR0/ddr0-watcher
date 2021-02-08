use reqwest;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio;

const VERSION: &str = "0.1.0";

const URLS: [&str; 3] = [
	"https://ddr0.ca/gallery.html",
	"https://ddr0.ca/⚂/",
	"https://ddr0.ca/⚂/ws/socket.io.js",
];

struct Query<'a> {
	url: &'a str,
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
	for &url in URLS.iter() {
		let start = Instant::now();
		let res = client.get(url).send().await;
		//println!("res in {}ms: {:?}", duration.as_millis(), res);

		let query = Query {
			url,
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
		query.url,
	);
}
