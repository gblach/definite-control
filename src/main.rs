//  This Source Code Form is subject to the terms of the Mozilla Public
//  License, v. 2.0. If a copy of the MPL was not distributed with this
//  file, You can obtain one at http://mozilla.org/MPL/2.0/.

mod command;
mod service;
mod socket;
mod table;
use argh::FromArgs;

#[derive(FromArgs, Debug)]
/// Control - service supervisor.
pub struct Args {
	/// command
	#[argh(subcommand)]
	command: Option<Command>,
}

#[derive(FromArgs, Debug)]
#[argh(subcommand)]
enum Command {
	StartAll(StartAll),
	StopAll(StopAll),
	RestartAll(RestartAll),
	Start(Start),
	Stop(Stop),
	Restart(Restart),
	Reload(Reload),
	Enable(Enable),
	Disable(Disable),
	Status(Status),
	Check(Check),
}

#[derive(FromArgs, Debug)]
/// Start enabled services.
#[argh(subcommand, name="start-all")]
struct StartAll {}

#[derive(FromArgs, Debug)]
/// Stop running services.
#[argh(subcommand, name="stop-all")]
struct StopAll {}

#[derive(FromArgs, Debug)]
/// Restart supervisor.
#[argh(subcommand, name="restart-all")]
struct RestartAll {}

#[derive(FromArgs, Debug)]
/// Start service(s).
#[argh(subcommand, name="start")]
struct Start {
	#[argh(positional)]
	/// service name
	service_names: Vec<String>,
}

#[derive(FromArgs, Debug)]
/// Stop service(s).
#[argh(subcommand, name="stop")]
struct Stop {
	#[argh(positional)]
	/// service name
	service_names: Vec<String>,
}

#[derive(FromArgs, Debug)]
/// Restart service(s).
#[argh(subcommand, name="restart")]
struct Restart {
	#[argh(positional)]
	/// service name
	service_names: Vec<String>,
}

#[derive(FromArgs, Debug)]
/// Reload service(s).
#[argh(subcommand, name="reload")]
struct Reload {
	#[argh(positional)]
	/// service name
	service_names: Vec<String>,
}

#[derive(FromArgs, Debug)]
/// Enable service(s).
#[argh(subcommand, name="enable")]
struct Enable {
	#[argh(positional)]
	/// service name
	service_names: Vec<String>,
}

#[derive(FromArgs, Debug)]
/// Disable service(s).
#[argh(subcommand, name="disable")]
struct Disable {
	#[argh(positional)]
	/// service name
	service_names: Vec<String>,
}

#[derive(FromArgs, Debug)]
/// Show services status.
#[argh(subcommand, name="status")]
struct Status {
	#[argh(positional)]
	/// service name
	service_name: Option<String>,
}

#[derive(FromArgs, Debug)]
/// Check toml files syntax.
#[argh(subcommand, name="check")]
struct Check {
	#[argh(positional)]
	/// service name
	service_name: Option<String>,
}

fn main() {
	let args: Args = argh::from_env();
	match args.command {
		Some(Command::StartAll(_)) => command::start_all(),
		Some(Command::StopAll(_)) => command::stop_all(),
		Some(Command::RestartAll(_)) => command::restart_all(),
		Some(Command::Start(args1)) => command::start(args1.service_names),
		Some(Command::Stop(args1)) => command::stop(args1.service_names),
		Some(Command::Restart(args1)) => command::restart(args1.service_names),
		Some(Command::Reload(args1)) => command::reload(args1.service_names),
		Some(Command::Enable(args1)) => command::enable(args1.service_names),
		Some(Command::Disable(args1)) => command::disable(args1.service_names),
		Some(Command::Status(args1)) => command::status(args1.service_name),
		Some(Command::Check(args1)) => command::check(args1.service_name),
		None => command::status(None),
	}
}
