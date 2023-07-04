//  This Source Code Form is subject to the terms of the Mozilla Public
//  License, v. 2.0. If a copy of the MPL was not distributed with this
//  file, You can obtain one at http://mozilla.org/MPL/2.0/.

use super::{command, socket};
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::fs;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Service {
	pub control: Control,
	process: Process,
	system: Option<System>,
	env: Option<HashMap<String, String>>,
}

#[derive(Deserialize, Debug)]
pub struct Control {
	#[serde(alias="descr")]
	_descr: String,
	#[serde(default="xvec")]
	depends: Vec<String>,
	#[serde(alias="one-time", default="xfalse")]
	pub one_time: bool,
	#[serde(default="xfalse")]
	restart: bool,
	#[serde(alias="restart-always", default="xfalse")]
	restart_always: bool,
}

#[derive(Deserialize, Debug)]
struct Process {
	#[serde(alias="start-cmd")]
	start_cmd: Vec<String>,
	#[serde(alias="stop-cmd")]
	stop_cmd: Option<Vec<String>>,
	#[serde(alias="stop-sig", default="sigterm")]
	stop_sig: i32,
	#[serde(alias="restart-cmd")]
	restart_cmd: Option<Vec<String>>,
	#[serde(alias="restart-sig")]
	restart_sig: Option<i32>,
	#[serde(alias="reload-cmd")]
	reload_cmd: Option<Vec<String>>,
	#[serde(alias="reload-sig", default="sighup")]
	reload_sig: i32,
}

#[derive(Deserialize, Debug)]
struct System {
	user: Option<String>,
	group: Option<String>,
	workdir: Option<String>,
}

fn xvec() -> Vec<String> {
	vec![]
}

fn xfalse() -> bool {
	false
}

fn sighup() -> i32 {
	libc::SIGHUP
}

fn sigterm() -> i32 {
	libc::SIGTERM
}

type ResultService = Result<Service, toml::de::Error>;

#[derive(Debug)]
pub struct Meta {
	pub exists: bool,
	pub valid: bool,
	pub enabled: bool,
	pub running: bool,
	pub service: ResultService,
	pub pid: Option<i32>,
}

pub fn confdir() -> String {
	let euid = unsafe { libc::geteuid() };
	if euid == 0 {
		String::from("/etc/control")
	} else {
		let home = unsafe {
			let pw = libc::getpwuid(euid);
			CStr::from_ptr((*pw).pw_dir).to_string_lossy()
		};
		format!("{home}/.control")
	}
}

pub fn confdir_enabled() -> String {
	format!("{}/enabled", confdir())
}

pub fn rundir() -> String {
	let euid = unsafe { libc::geteuid() };
	if euid == 0 {
		String::from("/run/control")
	} else {
		let home = unsafe {
			let pw = libc::getpwuid(euid);
			CStr::from_ptr((*pw).pw_dir).to_string_lossy()
		};
		format!("{home}/.control/run")
	}
}

pub fn control_lock() -> String {
	format!("{}/control.lock", rundir())
}

pub fn control_sock() -> String {
	format!("{}/control.sock", rundir())
}

pub fn pidfile(service_name: &str) -> String {
	format!("{}/{service_name}.pid", rundir())
}

pub fn pidfile_put(service_name: &str, pid: i32) -> String {
	let pidfile = pidfile(service_name);
	let pid = format!("{}\n", pid);
	fs::write(&pidfile, pid).unwrap();
	pidfile
}

pub fn pidfile_get(service_name: &str) -> Option<i32> {
	let pidfile = pidfile(service_name);
	fs::read_to_string(pidfile).unwrap_or("".into()).trim_end().parse().ok()
}

pub fn pidfile_del(service_name: &str) {
	let pidfile = pidfile(service_name);
	fs::remove_file(pidfile).unwrap();
}

pub fn load(service_name: &str) -> ResultService {
	let service_file = format!("{}/{service_name}.toml", confdir());
	let service_file = fs::read_to_string(service_file).unwrap_or(String::new());
	toml::from_str(&service_file)
}

pub fn meta(service_name: &str) -> Meta {
	let service_file = format!("{}/{service_name}.toml", confdir());
	let exists = PathBuf::from(service_file).exists();
	let service = load(service_name);
	let valid = service.is_ok();

	let enabled = if exists && valid {
		let service_file = format!("{}/{service_name}.toml", confdir_enabled());
		PathBuf::from(service_file).exists()
	} else {
		false
	};

	let pid = if exists && valid {
		pidfile_get(service_name)
	} else {
		None
	};

	let running = pid.is_some();

	Meta { exists, valid, enabled, running, service, pid }
}

pub fn order(service_names: Vec<String>) -> Vec<String> {
	let mut services: HashMap<String, Vec<String>>
		= HashMap::with_capacity(service_names.len());

	for service_name in service_names {
		let service = load(&service_name);
		if let Ok(service) = service {
			let depends = service.control.depends;
			services.insert(service_name.to_string(), depends);
		}
	}

	let mut order: Vec<String> = Vec::with_capacity(services.len());

	while order.len() < order.capacity() {
		let len = order.len();

		for service_name in services.keys() {
			let depends = services.get(service_name).unwrap();
			let depends: Vec<_> = depends.iter().cloned()
				.filter(|e| services.contains_key(e)).collect();
			if depends.iter().all(|e| order.contains(e)) {
				order.push(service_name.to_string());
			}
		}

		for service_name in &order[len..] {
			services.remove(service_name);
		}
	}

	order
}

#[derive(PartialEq)]
pub enum Error {
	CannotSpawn,
	CannotKill,
	NotFound,
	NoDaemon,
}

fn spawn_start(service: &Service) -> Result<i32, Error> {
	let mut process = Command::new(&service.process.start_cmd[0]);
	process.args(&service.process.start_cmd[1..]);

	if let Some(system) = &service.system {
		let euid = unsafe { libc::geteuid() };

		if euid == 0 {
			let str_into_raw = |str: &str| -> *const i8 {
				CString::new(String::from(str)).unwrap().into_raw()
			};

			if let Some(user) = &system.user {
				unsafe {
					let user = libc::getpwnam(str_into_raw(user));
					if ! user.is_null() {
						process.uid((*user).pw_uid);
					}
				};
			}

			if let Some(group) = &system.group {
				unsafe {
					let group = libc::getgrnam(str_into_raw(group));
					if ! group.is_null() {
						process.gid((*group).gr_gid);
					}
				};
			}
		}

		if let Some(workdir) = &system.workdir {
			process.current_dir(workdir);
		}
	}

	if let Some(env) = &service.env {
		process.envs(env);
	};

	let child = process.spawn();

	if let Ok(mut child) = child {
		if service.control.one_time {
			child.wait().unwrap();
			return Err(Error::NotFound);
		}
		return Ok(child.id() as i32);
	}

	Err(Error::CannotSpawn)
}

pub fn start(service_name: &str, children: &Arc<Mutex<command::Children>>) -> Result<i32, Error> {
	let service = load(service_name);

	if let Ok(service) = service {
		let pid = spawn_start(&service);

		if let Ok(pid) = pid {
			pidfile_put(service_name, pid);
			let mut children_ref = children.lock().unwrap();
			let control = service.control;
			let child = (service_name.into(), control.restart, control.restart_always);
			children_ref.insert(pid, child);
		}

		return pid
	}

	Err(Error::NotFound)
}

pub fn start_socket(service_name: &str) -> Result<(), Error> {
	let cmdline = format!("start {service_name}");
	let pid = socket::socket_chat(&cmdline);

	if let Ok(pid) = pid {
		if pid == 0 {
			return Err(Error::NotFound);
		}
		return Ok(());
	}

	Err(Error::NoDaemon)
}

fn spawn(command: &[String]) -> Result<(), Error> {
	let child = Command::new(&command[0]).args(&command[1..]).spawn();
	if child.is_ok() {
		Ok(())
	} else {
		Err(Error::CannotSpawn)
	}
}

fn kill(pid: i32, sig: i32) -> Result<(), Error> {
	let err = unsafe { libc::kill(pid, sig) };
	if err == 0 {
		Ok(())
	} else {
		Err(Error::CannotKill)
	}
}

pub fn stop(service: &Service, pid: Option<i32>) -> Result<(), Error> {
	if let Some(stop_cmd) = &service.process.stop_cmd {
		return spawn(stop_cmd);
	} else if let Some(pid) = pid {
		return kill(pid, service.process.stop_sig);
	}

	Err(Error::NotFound)
}

pub fn restart(service: &Service, pid: i32) -> Result<(), Error> {
	if let Some(restart_cmd) = &service.process.restart_cmd {
		return spawn(restart_cmd);
	} else if let Some(restart_sig) = service.process.restart_sig {
		return kill(pid, restart_sig);
	}

	Err(Error::NotFound)
}

pub fn reload(service: &Service, pid: i32) -> Result<(), Error> {
	if let Some(reload_cmd) = &service.process.reload_cmd {
		spawn(reload_cmd)
	} else {
		kill(pid, service.process.reload_sig)
	}
}
