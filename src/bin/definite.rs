//  This Source Code Form is subject to the terms of the Mozilla Public
//  License, v. 2.0. If a copy of the MPL was not distributed with this
//  file, You can obtain one at http://mozilla.org/MPL/2.0/.

use std::env;
use std::ffi::CString;
use std::fs;
use std::io::{BufReader, BufRead};
use std::process;

#[derive(PartialEq, Debug)]
enum Mode {
	Definite,
	Halt,
	Reboot,
}

static mut REBOOT_CMD: i32 = libc::LINUX_REBOOT_CMD_RESTART;

extern fn on_reboot(signal: libc::c_int) {
	unsafe {
		REBOOT_CMD = if signal == libc::SIGTERM {
			libc::LINUX_REBOOT_CMD_POWER_OFF
		} else {
			libc::LINUX_REBOOT_CMD_RESTART
		};

		libc::alarm(30);
	}

	let mut child = process::Command::new("/bin/control").arg("stop-all").spawn().unwrap();
	child.wait().unwrap();

	unsafe { libc::kill(-1, libc::SIGTERM); }
}

extern fn on_alarm(_signal: libc::c_int) {
	unsafe { libc::kill(-1, libc::SIGKILL); }
}

fn list_filesystems() -> Vec<String> {
	let mut filesystems = vec![];

	let file = fs::File::open("/proc/mounts").unwrap();
	let reader = BufReader::new(file);
	for line in reader.lines().flatten() {
		let mountpoint = line.split(' ').nth(1).unwrap();
		filesystems.push(mountpoint.into());
	}

	filesystems
}

fn mount(src: &str, target: &str, fstype: &str, flags: u64, data: &str) {
	let src = CString::new(src).unwrap().into_raw();
	let target = CString::new(target).unwrap().into_raw();
	let fstype = CString::new(fstype).unwrap().into_raw();
	let flags = libc::MS_NOEXEC | libc::MS_NOSUID | flags;
	let data = CString::new(data).unwrap().into_raw();

	unsafe {
		libc::mount(src, target, fstype, flags, data as *const libc::c_void);
	}
}

fn mount_filesystems() {
	mount("proc", "/proc", "proc", libc::MS_NODEV, "");

	if ! list_filesystems().contains(&String::from("/dev")) {
		mount("devtmpfs", "/dev", "devtmpfs", 0, "");
	} else {
		mount("devtmpfs", "/dev", "devtmpfs", libc::MS_REMOUNT, "");
	}

	fs::create_dir("/dev/pts").unwrap();
	mount("devpts", "/dev/pts", "devpts", 0, "gid=5,mode=620");

	fs::create_dir("/dev/shm").unwrap();
	mount("tmpfs", "/dev/shm", "tmpfs", 0, "mode=0777");

	mount("sysfs", "/sys", "sysfs", libc::MS_NODEV, "");
	mount("tmpfs", "/run", "tmpfs", libc::MS_NODEV, "mode=0755");
	mount("tmpfs", "/tmp", "tmpfs", libc::MS_NODEV, "mode=1777");
}

fn umount_filesystems() {
	for target in list_filesystems() {
		let target = CString::new(target).unwrap().into_raw();
		let err = unsafe { libc::umount(target) };

		if err < 0 {
			let empty = CString::new("").unwrap().into_raw();
			let flags = libc::MS_REMOUNT | libc::MS_RDONLY;
			unsafe {
				libc::mount(empty, target, empty, flags, std::ptr::null());
			}
		}
	}
}

fn definite() {
	if process::id() != 1 {
		println!("Process ID is not 1");
		process::exit(1);
	}

	mount_filesystems();

	process::Command::new("/bin/control").arg("start-all").spawn().unwrap();

	unsafe {
		let mut sigset: libc::sigset_t = std::mem::zeroed();
		libc::sigfillset(&mut sigset);
		libc::sigdelset(&mut sigset, libc::SIGTERM);
		libc::sigdelset(&mut sigset, libc::SIGUSR1);
		libc::sigdelset(&mut sigset, libc::SIGALRM);

		libc::sigprocmask(libc::SIG_BLOCK, &sigset, std::ptr::null_mut());

		libc::signal(libc::SIGTERM, on_reboot as extern fn(libc::c_int) as usize);
		libc::signal(libc::SIGUSR1, on_reboot as extern fn(libc::c_int) as usize);
		libc::signal(libc::SIGALRM, on_alarm as extern fn(libc::c_int) as usize);
	}

	loop {
		let pid = unsafe {
			let mut wstatus: libc::c_int = 0;
			libc::wait(&mut wstatus)
		};
		if pid < 0 {
			break;
		}
	}

	unsafe { libc::sync(); }
	umount_filesystems();

	unsafe { libc::reboot(REBOOT_CMD); }
}

fn reboot(signal: i32) {
	unsafe { libc::kill(1, signal); }
}

fn main() {
	let mut mode = Mode::Definite;

	for arg in env::args_os().take(2) {
		let arg = arg.to_string_lossy();
		let arg = arg.rsplit('/').next().unwrap();
		mode = match arg {
			"halt" => Mode::Halt,
			"reboot" => Mode::Reboot,
			&_ => Mode::Definite,
		};
		if mode != Mode::Definite {
			break;
		}
	}

	match mode {
		Mode::Definite => definite(),
		Mode::Halt => reboot(libc::SIGTERM),
		Mode::Reboot => reboot(libc::SIGUSR1),
	}
}
