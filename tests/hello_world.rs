use profiler::{Profiler, new_frame, scope, custom_scope};
use profiler_attributes::profile;

#[profile]
fn work() -> i32 {
	std::thread::sleep(std::time::Duration::from_millis(10));

	5
}

#[test]
fn simple() {
	for _ in 0..10 {
		assert_eq!(work(), 5);

		new_frame!();
	}
	for _ in 0..10 {
		{
			scope!("scope_task");
            std::thread::sleep(std::time::Duration::from_millis(10));
		}

		new_frame!();
	}
}


#[test]
fn custom() {
	let mut profiler = Profiler::new();

	for _ in 0..10 {
		{
			custom_scope!(&mut profiler, "scope_task");
			std::thread::sleep(std::time::Duration::from_millis(10));
		}
		profiler.new_frame();
	}
}