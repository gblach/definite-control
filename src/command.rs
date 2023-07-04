//  This Source Code Form is subject to the terms of the Mozilla Public
//  License, v. 2.0. If a copy of the MPL was not distributed with this
//  file, You can obtain one at http://mozilla.org/MPL/2.0/.

use super::{service, socket, table};
use table::*;
use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs as ufs;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::process;
use std::sync::{Arc, Mutex};
use std::{thread, time};

static mut BREAK_START_ALL_LOOP: bool = false;

extern fn on_sigterm(_signal: libc::c_int) {
	unsafe { BREAK_START_ALL_LOOP = true; }
}

fn list_directory(directory: String, extension: &str) -> Vec<String> {
	let mut files = Vec::new();

	for entry in fs::read_dir(directory).unwrap() {
		let path = entry.unwrap().path();
		if path.extension().unwrap_or(&OsString::new()) == extension {
			files.push(path.file_stem().unwrap().to_string_lossy().to_string());
		}
	}

	files
}

pub type Children = HashMap<i32, (String, bool, bool)>;

pub fn start_all() {
	if ! Path::new(&service::rundir()).exists() {
		fs::create_dir(service::rundir()).unwrap();
	}

	let lockfile = service::control_lock();
	let lock = fs::File::create(&lockfile).unwrap();
	unsafe {
		if libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) < 0 {
			return table_err("control", "Already running");
		}

		if libc::fork() != 0 {
			process::exit(0);
		}
	}

	let pid = format!("{}\n", process::id());
	fs::write(&lockfile, pid).unwrap();

	unsafe {
		libc::signal(libc::SIGTERM, on_sigterm as extern fn(libc::c_int) as usize);
	}

	let children = Arc::new(Mutex::new(Children::new()));

	let mut service_names: Vec<String> = Vec::new();
	for service_name in list_directory(service::confdir_enabled(), "toml") {
		service_names.push(service_name);
	}

	let service_names = service::order(service_names);
	for service_name in service_names {
		log_bold("Starting", &service_name);
		let _ = service::start(&service_name, &children);
	}

	socket::listen(&children);

	loop {
		let (pid, dirty) = unsafe {
			let mut wstatus: libc::c_int = 0;
			let pid = libc::wait(&mut wstatus);

			if pid < 0 {
				thread::sleep(time::Duration::from_millis(100));
				if BREAK_START_ALL_LOOP { break; } else { continue; }
			}

			let dirty = {
				if libc::WIFEXITED(wstatus) {
					libc::WEXITSTATUS(wstatus) != 0
				} else if libc::WIFSIGNALED(wstatus) {
					libc::WCOREDUMP(wstatus)
				} else {
					false
				}
			};

			(pid, dirty)
		};

		let mut children_ref = children.lock().unwrap();
		let child = children_ref.remove_entry(&pid).unwrap().1;
		let (service_name, restart, restart_always) = child;
		service::pidfile_del(&service_name);
		drop(children_ref);

		if (dirty && restart) || restart_always {
			let _ = service::start(&service_name, &children);
		}
	}

	socket::socket_del();
	fs::remove_file(lockfile).unwrap();
}

pub fn stop_all() {
	let mut service_names: Vec<String> = Vec::new();
	for service_name in list_directory(service::confdir_enabled(), "toml") {
		service_names.push(service_name);
	}
	for service_name in list_directory(service::rundir(), "pid") {
		if ! service_names.contains(&service_name) {
			service_names.push(service_name);
		}
	}

	let mut service_names = service::order(service_names);
	service_names.reverse();

	for service_name in service_names {
		let meta = service::meta(&service_name);
		let service = meta.service.unwrap();

		if service.control.one_time || meta.running {
			log_bold("Stopping", &service_name);

			let cmdline = format!("stop {service_name}");
			let pid = socket::socket_chat(&cmdline);

			if pid.is_ok() {
				let _result = service::stop(&service, meta.pid);
			}
		}
	}

	let lockfile = service::control_lock();
	let pid = fs::read_to_string(lockfile).unwrap_or("".into()).trim_end().parse();
	if let Ok(pid) = pid {
		unsafe { libc::kill(pid, libc::SIGTERM); }
	}

	let lockfile: PathBuf = service::control_lock().into();
	while lockfile.exists() {
		thread::sleep(time::Duration::from_millis(10));
	}
}

pub fn restart_all() {
	stop_all();
	start_all();
}

pub fn start(service_names: Vec<String>) {
	let mut table = Table::new();

	for service_name in service_names {
		table.first(&service_name);

		let meta = service::meta(&service_name);

		if ! meta.exists {
			table.field("Service not exists", RED);
			continue;
		}

		if ! meta.valid {
			table.field("Invalid service", RED);
			continue;
		}

		if meta.running {
			table.field("Already running", YELLOW);
			continue;
		}

		let result = service::start_socket(&service_name);

		if let Err(err) = result {
			if err == service::Error::NotFound {
				table.field("Cannot start", RED);
				continue;
			}

			if err == service::Error::NoDaemon {
				return table_err("control", "Daemon is not running");
			}
		};

		table.field("Started", GREEN);
	}

	table.print();
}

pub fn stop(service_names: Vec<String>) {
	let mut table = Table::new();

	for service_name in service_names {
		table.first(&service_name);

		let meta = service::meta(&service_name);

		if ! meta.exists {
			table.field("Service not exists", RED);
			continue;
		}

		if ! meta.valid {
			table.field("Invalid service", RED);
			continue;
		}

		let service = meta.service.unwrap();

		if ! service.control.one_time && ! meta.running {
			table.field("Not running", YELLOW);
			continue;
		}

		let cmdline = format!("stop {service_name}");
		let pid = socket::socket_chat(&cmdline);

		if pid.is_err() {
			return table_err("control", "Daemon is not running");
		}

		let result = service::stop(&service, meta.pid);
		if result.is_ok() {
			table.field("Stopped", GREEN);
		} else {
			table.field("Cannot stop", RED);
		}
	}

	table.print();
}

pub fn restart(service_names: Vec<String>) {
	let mut table = Table::new();

	for service_name in service_names {
		table.first(&service_name);

		let meta = service::meta(&service_name);

		if ! meta.exists {
			table.field("Service not exists", RED);
			continue;
		}

		if ! meta.valid {
			table.field("Invalid service", RED);
			continue;
		}

		if ! meta.running {
			table.field("Not running", YELLOW);
			continue;
		}

		let service = meta.service.unwrap();
		let pid = meta.pid.unwrap();
		let result = service::restart(&service, pid);

		if let Err(err) = result {
			if err != service::Error::NotFound {
				table.field("Cannot restart", RED);
				continue;
			}

			let result = service::stop(&service, meta.pid);
			if result.is_err() {
				table.field("Cannot stop", RED);
				continue;
			}

			let pidfile: PathBuf = service::pidfile(&service_name).into();
			while pidfile.exists() {
				thread::sleep(time::Duration::from_millis(10));
			}

			let result = service::start_socket(&service_name);
			if result.is_err() {
				table.field("Cannot start", RED);
				continue;
			}
		}

		table.field("Restarted", GREEN);
	}

	table.print();
}

pub fn reload(service_names: Vec<String>) {
	let mut table = Table::new();

	for service_name in service_names {
		table.first(&service_name);

		let meta = service::meta(&service_name);

		if ! meta.exists {
			table.field("Service not exists", RED);
			continue;
		}

		if ! meta.valid {
			table.field("Invalid service", RED);
			continue;
		}

		if ! meta.running {
			table.field("Not running", YELLOW);
			continue;
		}

		let service = meta.service.unwrap();
		let pid = meta.pid.unwrap();
		let result = service::reload(&service, pid);

		if result.is_ok() {
			table.field("Reloaded", GREEN);
		} else {
			table.field("Cannot reload", RED);
		}
	}

	table.print();
}

pub fn enable(service_names: Vec<String>) {
	let mut table = Table::new();

	for service_name in service_names {
		table.first(&service_name);

		let meta = service::meta(&service_name);

		if ! meta.exists {
			table.field("Service not exists", RED);
			continue;
		}

		if ! meta.valid {
			table.field("Invalid service", RED);
			continue;
		}

		if meta.enabled {
			table.field("Already enabled", YELLOW);
			continue;
		}

		if ! Path::new(&service::confdir_enabled()).exists() {
			fs::create_dir(service::confdir_enabled()).unwrap();
		}

		let service_from = format!("../{service_name}.toml");
		let service_to = format!("{}/{service_name}.toml", service::confdir_enabled());

		match ufs::symlink(service_from, service_to) {
			Ok(_) => table.field("Enabled", GREEN),
			Err(err) => table.field(&err.to_string(), RED),
		};
	}

	table.print();
}

pub fn disable(service_names: Vec<String>) {
	let mut table = Table::new();

	for service_name in service_names {
		table.first(&service_name);

		let meta = service::meta(&service_name);

		if ! meta.exists {
			table.field("Service not exists", RED);
			continue;
		}

		if ! meta.enabled {
			table.field("Already disabled", YELLOW);
			continue;
		}

		let service_file = format!("{}/{service_name}.toml", service::confdir_enabled());

		match fs::remove_file(service_file) {
			Ok(_) => table.field("Disabled", GREEN),
			Err(err) => table.field(&err.to_string(), RED),
		};
	}

	table.print();
}

pub fn status(service_name: Option<String>) {
	let mut service_names: Vec<String> = Vec::new();

	if let Some(service_name) = service_name {
		service_names.push(service_name);
	} else {
		for service_name in list_directory(service::confdir(), "toml") {
			service_names.push(service_name);
		}
	}

	service_names.sort();

	let mut table = Table::new();

	for service_name in service_names {
		table.first(&service_name);

		let meta = service::meta(&service_name);

		if ! meta.exists {
			table.field("Not exists", RED).empty(1);
			continue;
		}

		if ! meta.valid {
			table.field("Invalid", RED).empty(1);
			continue;
		}

		if meta.enabled {
			table.field("Enabled", GREEN);
		} else {
			table.field("Disabled", YELLOW);
		}

		let service = meta.service.unwrap();

		if service.control.one_time {
			table.field("One time", GREEN);
		} else if meta.running {
			table.field("Running", GREEN);
		} else {
			table.field("Not running", YELLOW);
		}
	}

	table.print();
}

pub fn check(service_name: Option<String>) {
	if let Some(service_name) = service_name {
		let meta = service::meta(&service_name);

		if ! meta.exists {
			return table_err(&service_name, "Not exists");
		}

		let check = match meta.service {
			Ok(service) => format!("{:#?}\n", service),
			Err(err) => err.to_string(),
		};

		print!("\n  {}\n", check.replace('\n', "\n  "));
	} else {
		let mut table = Table::new();

		let mut service_names = list_directory(service::confdir(), "toml");
		service_names.sort();

		for service_name in service_names {
			let confdir = format!("{}/", service::confdir());
			table.ppfirst(&confdir, &service_name, ".toml");

			let meta = service::meta(&service_name);
			match meta.valid {
				true => table.field("OK", GREEN),
				false => table.field("Invalid", RED),
			};
		}

		table.print();
	}
}
