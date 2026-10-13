//! App-level tests (core tag and lyrics tests live in `crates/deepkick-core/tests`).

#[test]
fn report_json_roundtrip() {
	use crate::state::*;
	let mut queue = QueueState::default();
	queue.items.push(Item { id: 1, uri: "x".into(), name: "x.mp3".into(), folder: "".into(), size: 0, status: Status::Done, artist: None, title: None, detail: None });
	queue.run = Some(Run { mode: Mode::Fetch, active: false, cancelled: false, started_at: 1, finished_at: Some(2), current: 1, total: 1 });
	let report = Report::from_queue(&queue).unwrap();
	let json = serde_json::to_string(&report).unwrap();
	println!("{json}");
	let back: Report = serde_json::from_str(&json).unwrap();
	assert_eq!(back.summary.get(&Status::Done), Some(&1));
	let value = serde_json::to_value(&report).unwrap();
	assert!(value["summary"]["done"].is_number());
}
