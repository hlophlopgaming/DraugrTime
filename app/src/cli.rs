use chrono::{Datelike, Days, Local, NaiveDate, TimeZone};
use draugr_time_core::{GroupBy, Storage};
use std::{
	collections::HashMap,
	fs,
	io::{Read, Write},
	path::PathBuf,
	time::Duration,
};
const HELP:&str="Draugr Time CLI (local, no graphical session required)\n\nUsage: draugr-time-cli [--data-dir FOLDER] COMMAND [OPTIONS]\n  status [--json]                         Live localhost tracker status\n  report --from START --to END            Active-time report\n    [--group tag|application|document] [--tag ID] [--json|--format csv|json|text]\n  export --from START --to END [--format json|csv] [--group tag|application|document]\n  import FILE                            Import Draugr JSON or ActivityWatch dump\n\nSTART/END: Unix UTC milliseconds (exclusive end), or YYYY-MM-DD local dates\n(inclusive end date). JSON export is a restorable archive; CSV is a report.\nOutput goes to stdout; errors to stderr with nonzero exit status.\n";
fn error(message: &str) -> String {
	message.into()
}
pub fn run(args: Vec<String>) -> Result<String, String> {
	if args.iter().any(|a| a == "--help" || a == "-h") {
		return Ok(if cfg!(target_os = "windows") {
			HELP.into()
		} else {
			HELP.replace("draugr-time-cli", "draugr-time")
		});
	}
	let mut options = HashMap::new();
	let mut positional = Vec::new();
	let mut json = false;
	let mut iter = args.iter();
	while let Some(a) = iter.next() {
		if a == "--json" {
			json = true;
			continue;
		}
		if a.starts_with("--") {
			if ![
				"--data-dir",
				"--from",
				"--to",
				"--group",
				"--tag",
				"--format",
			]
			.contains(&a.as_str())
			{
				return Err(format!("unknown option {a}"));
			}
			let value = iter
				.next()
				.ok_or_else(|| format!("missing value for {a}"))?;
			if options.insert(a.as_str(), value.as_str()).is_some() {
				return Err(format!("duplicate option {a}"));
			}
		} else {
			positional.push(a.as_str());
		}
	}
	let command = *positional
		.first()
		.ok_or_else(|| error("expected a command; use --help"))?;
	if !["status", "report", "export", "import"].contains(&command) {
		return Err(format!("unknown command {command}"));
	}
	if positional.len() != if command == "import" { 2 } else { 1 } {
		return Err(error("unexpected or missing positional argument"));
	}
	for &option in options.keys() {
		if option == "--data-dir" {
			continue;
		}
		let allowed = match command {
			"report" => true,
			"export" => option != "--tag",
			"status" | "import" => false,
			_ => false,
		};
		if !allowed {
			return Err(format!("{option} is not valid for {command}"));
		}
	}
	if json && options.contains_key("--format") {
		return Err(error("use either --json or --format"));
	}
	let format = if json {
		"json"
	} else {
		options
			.get("--format")
			.copied()
			.unwrap_or(if command == "export" { "json" } else { "text" })
	};
	if !["json", "csv", "text"].contains(&format) || (command == "export" && format == "text") {
		return Err(error("invalid format"));
	}
	if command == "import" && json {
		return Err(error("--json is not valid for import"));
	}
	let directory = options
		.get("--data-dir")
		.map(PathBuf::from)
		.map(Ok)
		.unwrap_or_else(draugr_time_daemon::paths::data_dir)
		.map_err(|e| e.to_string())?;
	if command == "status" {
		let status = live_status(&directory)?;
		return if json {
			serde_json::to_string_pretty(&status).map_err(|e| e.to_string())
		} else {
			Ok(status["state"].as_str().unwrap_or("unknown").to_owned())
		};
	}
	let from = if command == "import" {
		0
	} else {
		bound(
			options
				.get("--from")
				.copied()
				.ok_or_else(|| error("--from is required"))?,
			false,
		)?
	};
	let to = if command == "import" {
		1
	} else {
		bound(
			options
				.get("--to")
				.copied()
				.ok_or_else(|| error("--to is required"))?,
			true,
		)?
	};
	if to <= from {
		return Err(error("expected 0 <= from < to"));
	}
	let group = GroupBy::parse(options.get("--group").copied().unwrap_or("tag"))
		.ok_or_else(|| error("invalid grouping"))?;
	let tag = options
		.get("--tag")
		.map(|s| {
			s.parse::<i64>()
				.ok()
				.filter(|id| *id > 0)
				.ok_or_else(|| error("invalid tag id"))
		})
		.transpose()?;
	let path = directory.join("activity.sqlite");
	if command == "import" {
		fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
	} else if !path.is_file() {
		return Err(error("history database does not exist"));
	}
	let mut db = Storage::open(path).map_err(|e| e.to_string())?;
	if command == "import" {
		let bytes = fs::read(positional[1]).map_err(|e| e.to_string())?;
		let count = db.import_json(&bytes).map_err(|e| e.to_string())?;
		return Ok(format!("Imported {count} events"));
	}
	if command == "export" && format == "json" {
		return serde_json::to_string_pretty(
			&db.export_archive(from, to).map_err(|e| e.to_string())?,
		)
		.map_err(|e| e.to_string());
	}
	let report = db.report(from, to).map_err(|e| e.to_string())?;
	match format {
        "csv"=>Ok(report.csv(group,tag)),
        "json"=>serde_json::to_string_pretty(&serde_json::json!({"from":from,"to":to,"active_ms":report.rows(group,tag).iter().map(|r|r.duration_ms).sum::<i64>(),"rows":report.rows(group,tag)})).map_err(|e|e.to_string()),
        _=>Ok(report.rows(group,tag).iter().map(|r|format!("{}\t{} ms\t{:.2}%",r.label,r.duration_ms,r.share*100.0)).collect::<Vec<_>>().join("\n")),
    }
}
fn bound(value: &str, end: bool) -> Result<i64, String> {
	if let Ok(ts) = value.parse::<i64>() {
		return if ts >= 0 {
			Ok(ts)
		} else {
			Err(error("negative timestamp"))
		};
	}
	let mut date = NaiveDate::parse_from_str(value, "%Y-%m-%d")
		.map_err(|_| error("use Unix milliseconds or YYYY-MM-DD"))?;
	if end {
		date = date
			.checked_add_days(Days::new(1))
			.ok_or_else(|| error("date overflow"))?;
	}
	Local
		.with_ymd_and_hms(date.year(), date.month(), date.day(), 0, 0, 0)
		.earliest()
		.map(|d| d.timestamp_millis())
		.filter(|ts| *ts >= 0)
		.ok_or_else(|| error("local midnight does not exist"))
}
fn live_status(directory: &std::path::Path) -> Result<serde_json::Value, String> {
	let token = fs::read_to_string(directory.join("api-token")).map_err(|e| e.to_string())?;
	if token
		.trim()
		.bytes()
		.any(|b| !b.is_ascii_alphanumeric() && b != b'-')
	{
		return Err(error("invalid API token"));
	}
	let mut stream = std::net::TcpStream::connect_timeout(
		&std::net::SocketAddr::from(([127, 0, 0, 1], draugr_time_daemon::paths::PORT)),
		Duration::from_secs(2),
	)
	.map_err(|e| format!("tracker unavailable: {e}"))?;
	stream
		.set_read_timeout(Some(Duration::from_secs(2)))
		.map_err(|e| e.to_string())?;
	stream
		.set_write_timeout(Some(Duration::from_secs(2)))
		.map_err(|e| e.to_string())?;
	write!(stream,"GET /status HTTP/1.0\r\nHost: localhost\r\nAuthorization: Bearer {}\r\nConnection: close\r\n\r\n",token.trim()).map_err(|e|e.to_string())?;
	let mut response = String::new();
	stream
		.take(1_048_577)
		.read_to_string(&mut response)
		.map_err(|e| e.to_string())?;
	if response.len() > 1_048_576 {
		return Err(error("status response too large"));
	}
	let (header, body) = response
		.split_once("\r\n\r\n")
		.ok_or_else(|| error("invalid status response"))?;
	if !header
		.lines()
		.next()
		.is_some_and(|s| s.split_whitespace().nth(1) == Some("200"))
	{
		return Err(error("local status request failed"));
	}
	serde_json::from_str(body).map_err(|e| e.to_string())
}
