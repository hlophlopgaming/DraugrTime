use std::{fs, process::Command};
fn command(dir: &std::path::Path) -> Command {
	#[cfg(not(target_os = "windows"))]
	let mut c = Command::new(env!("CARGO_BIN_EXE_draugr-time"));
	#[cfg(target_os = "windows")]
	let mut c = Command::new(env!("CARGO_BIN_EXE_draugr-time-cli"));
	c.env_remove("DISPLAY")
		.env_remove("WAYLAND_DISPLAY")
		.arg("--data-dir")
		.arg(dir);
	c
}
#[test]
fn cli_headless_help_report_export_import_and_errors() {
	let root = std::env::temp_dir().join(format!("draugr-cli-test-{}", std::process::id()));
	fs::create_dir_all(&root).unwrap();
	let source = root.join("source");
	fs::create_dir_all(&source).unwrap();
	let mut db = draugr_time_core::Storage::open(source.join("activity.sqlite")).unwrap();
	db.record_interval("afk", &serde_json::json!({"status":"active"}), 1000, 2000)
		.unwrap();
	db.record_interval(
		"window",
		&serde_json::json!({"app":"editor","title":"work"}),
		1000,
		2000,
	)
	.unwrap();
	let expected = db
		.report(0, 3000)
		.unwrap()
		.rows(draugr_time_core::GroupBy::Application, None);
	drop(db);
	assert!(command(&source)
		.arg("--help")
		.output()
		.unwrap()
		.status
		.success());
	let report = command(&source)
		.args([
			"report",
			"--from",
			"0",
			"--to",
			"3000",
			"--group",
			"application",
			"--json",
		])
		.output()
		.unwrap();
	assert!(report.status.success(), "{:?}", report);
	let report: serde_json::Value = serde_json::from_slice(&report.stdout).unwrap();
	assert_eq!(report["rows"][0]["duration_ms"], expected[0].duration_ms);
	let csv = command(&source)
		.args([
			"report",
			"--from",
			"0",
			"--to",
			"3000",
			"--group",
			"application",
			"--format",
			"csv",
		])
		.output()
		.unwrap();
	assert!(String::from_utf8(csv.stdout)
		.unwrap()
		.contains(",\"editor\",1000,"));
	let export = command(&source)
		.args(["export", "--from", "0", "--to", "3000"])
		.output()
		.unwrap();
	assert!(export.status.success());
	let file = root.join("archive.json");
	fs::write(&file, &export.stdout).unwrap();
	let target = root.join("target");
	assert!(command(&target)
		.arg("import")
		.arg(&file)
		.output()
		.unwrap()
		.status
		.success());
	let roundtrip = command(&target)
		.args([
			"report",
			"--from",
			"0",
			"--to",
			"3000",
			"--group",
			"application",
			"--json",
		])
		.output()
		.unwrap();
	assert_eq!(
		report,
		serde_json::from_slice::<serde_json::Value>(&roundtrip.stdout).unwrap()
	);
	for args in [
		vec!["report", "--from", "bad"],
		vec!["nonsense"],
		vec!["report", "--group", "bogus"],
		vec!["import", "missing.json"],
	] {
		let output = command(&source).args(args).output().unwrap();
		assert!(!output.status.success());
		assert!(!output.stderr.is_empty());
		assert!(output.stdout.is_empty());
	}
	let status = command(&target)
		.args(["status", "--json"])
		.output()
		.unwrap();
	assert!(!status.status.success());
	assert!(!status.stderr.is_empty());
	fs::remove_dir_all(root).unwrap();
}
#[cfg(target_os = "linux")]
#[test]
fn local_date_reports_honor_short_and_long_dst_days() {
	use chrono::{TimeZone, Utc};
	let root = std::env::temp_dir().join(format!("draugr-cli-dst-{}", std::process::id()));
	fs::create_dir_all(&root).unwrap();
	let mut db = draugr_time_core::Storage::open(root.join("activity.sqlite")).unwrap();
	for (date, start, hours) in [
		(
			"2026-03-29",
			Utc.with_ymd_and_hms(2026, 3, 28, 23, 0, 0)
				.unwrap()
				.timestamp_millis(),
			23,
		),
		(
			"2026-10-25",
			Utc.with_ymd_and_hms(2026, 10, 24, 22, 0, 0)
				.unwrap()
				.timestamp_millis(),
			25,
		),
	] {
		db.record_interval(
			"afk",
			&serde_json::json!({"status":"active"}),
			start,
			start + hours * 3_600_000,
		)
		.unwrap();
		let output = command(&root)
			.env("TZ", "Europe/Berlin")
			.args(["report", "--from", date, "--to", date, "--json"])
			.output()
			.unwrap();
		assert!(output.status.success(), "{:?}", output);
		let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
		assert_eq!(result["active_ms"], hours * 3_600_000);
		assert_eq!(
			result["to"].as_i64().unwrap() - result["from"].as_i64().unwrap(),
			hours * 3_600_000
		);
	}
	drop(db);
	fs::remove_dir_all(root).unwrap();
}
