use std::{
	fs::{self, File, OpenOptions},
	io,
	path::Path,
};

pub(super) struct SingleInstanceLock {
	_file: File,
}

impl SingleInstanceLock {
	pub(super) fn acquire() -> io::Result<Option<Self>> {
		let directory = draugr_time_daemon::paths::default_dir()?;
		fs::create_dir_all(&directory)?;
		Self::at(&directory.join(".iced-instance.lock"))
	}

	fn at(path: &Path) -> io::Result<Option<Self>> {
		let file = OpenOptions::new()
			.create(true)
			.truncate(false)
			.write(true)
			.open(path)?;
		match file.try_lock() {
			Ok(()) => Ok(Some(Self { _file: file })),
			Err(std::fs::TryLockError::WouldBlock) => Ok(None),
			Err(std::fs::TryLockError::Error(error)) => Err(error),
		}
	}
}

#[cfg(test)]
mod tests {
	use std::fs;

	#[test]
	fn only_one_instance_can_hold_the_lock_until_it_exits() {
		let path = std::env::temp_dir().join(format!(
			"draugr-time-single-instance-{}.lock",
			std::process::id()
		));
		let first = super::SingleInstanceLock::at(&path).unwrap().unwrap();
		assert!(super::SingleInstanceLock::at(&path).unwrap().is_none());
		drop(first);
		assert!(super::SingleInstanceLock::at(&path).unwrap().is_some());
		fs::remove_file(path).unwrap();
	}
}
