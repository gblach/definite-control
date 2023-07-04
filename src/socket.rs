//  This Source Code Form is subject to the terms of the Mozilla Public
//  License, v. 2.0. If a copy of the MPL was not distributed with this
//  file, You can obtain one at http://mozilla.org/MPL/2.0/.

use super::{command, service};
use std::fs;
use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;

fn start(service_name: &str, children: &Arc<Mutex<command::Children>>) -> Option<i32> {
	let pid = service::pidfile_get(service_name);

	if pid.is_none() {
		let pid = service::start(service_name, children);
		return pid.ok();
	}

	pid
}

fn stop(service_name: &str, children: &Arc<Mutex<command::Children>>) -> Option<i32> {
	let pid = service::pidfile_get(service_name);

	if let Some(pid) = pid {
		let mut children_ref = children.lock().unwrap();
		let child = (service_name.into(), false, false);
		children_ref.insert(pid, child);
	}

	pid
}

pub fn listen(children: &Arc<Mutex<command::Children>>) {
	let children = Arc::clone(children);

	thread::spawn(move || {
		let sockfile = service::control_sock();
		if Path::new(&sockfile).exists() {
			fs::remove_file(&sockfile).unwrap();
		}

		let listener = UnixListener::bind(sockfile).unwrap();
		for socket in listener.incoming() {
			let mut socket = socket.unwrap();
			let mut cmdline = vec![0u8; 100];

			let len = socket.read(&mut cmdline).unwrap();
			cmdline.truncate(len);

			let cmdline = String::from_utf8(cmdline).unwrap();
			let mut cmdline = cmdline.split(' ');
			let command = cmdline.next().unwrap_or("");
			let service_name = cmdline.next().unwrap_or("");

			let pid = match command {
				"start" => start(service_name, &children),
				"stop" => stop(service_name, &children),
				&_ => None,
			};

			let pid = pid.unwrap_or(0).to_string();
			socket.write_all(pid.as_bytes()).unwrap();
		}
	});
}

pub fn socket_del() {
	let sockfile = service::control_sock();
	fs::remove_file(sockfile).ok();
}

pub fn socket_chat(cmdline: &str) -> std::io::Result<i32> {
	let sockfile = service::control_sock();
	let mut socket = UnixStream::connect(sockfile)?;

	socket.write_all(cmdline.as_bytes())?;

	let mut pid = vec![0u8; 20];
	let len = socket.read(&mut pid)?;
	pid.truncate(len);

	let pid = String::from_utf8(pid).unwrap();
	let pid: i32 = pid.parse().unwrap();

	Ok(pid)
}
