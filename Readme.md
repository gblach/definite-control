# Definite+Control
The Definite+Control is a set of two programs written in Rust that work together to boot and halt Linux system. Definite is a Linux init program. Control is a process supervisor.

Definite should be installed as `/sbin/definite` and Control should be installed as `/bin/control`.

## Definite
It is a good idea to create a symlink from `/sbin/definite` to `/sbin/init`. This is where Linux looks for the init program. Alternatively, the path to the init program can be overridden by a Linux cmd line.

### Boot stage
Initially, the /dev, /dev/pts, /dev/shm, /sys, /run and /proc filesystems are mounted. Each filesystem is mounted with NOEXEC and NOSUID flags. In addition /sys, /run and /proc are mounted with NODEV flag.

The next step is to start the Control supervisor  by calling `/bin/control start-all` command. After that the Definite switches to Wipe stage.

### Wipe stage
At this stage, all orphaned processes are wiped out.

When there are no processes left in the system except for the Definite process, the Definite switches to the Reboot stage. There are two convenient ways to make this switch:
1. By calling `/sbin/definite reboot` or `/sbin/definite halt` command.
2. By using symlinks from `/sbin/definite` to `/sbin/reboot` or `/sbin/halt`.

When one of the above commands is called:
1. First `/bin/control stop-all` is called.
2. Then SIGTERM is sent to all processes except for the Definite process.
3. If not all processes are terminated after 30 seconds, SIGKILL is sent to all processes except for the Definite process.

### Reboot stage
At this stage all filesystems are synced. Then all filesystems are unmounted or mounted as read-only. Finally `reboot(LINUX_REBOOT_CMD_RESTART)` or `reboot(LINUX_REBOOT_CMD_POWER_OFF)` is called.

## Control
The Control starts, stops, restarts and monitors processes defined in service files.

When the Control process is called by the root user, it looks for service files in the /etc/control directory, otherwise it looks in the ~/.control directory.

The following commands are recognized:

### control start-all
Starts the supervisor and all enabled services.

### control stop-all
Stops all monitored processes and the supervisor.

### control restart-all
Equivalent to calling `control stop-all` followed by `control start-all`.

### control start [<service_names...>]
Starts specified services, these services do not have to be enabled.

### control stop [<service_names...>]
Stops specified services.

### control restart [<service_names...>]
Restart specified services.

### control reload [<service_names...>]
Reload specified services.

### control enable [<service_names...>]
Enable specified services.

### control disable [<service_names...>]
Disable specified services.

### control status [<service_name>]
Displays the status of the specified service, or all services if no service is specified.

### control check [<service_name>]
Check the service file syntax of the specified service , or all services if no service is specified. When checking a single service, file the output is more verbose.

## Service file syntax
All service files are valid TOML files. The following fields are used:

```
[control]
# Brief description of what this service is for.
# This field is mandatory.
descr = "Brief description of the service"

# Soft dependencies used to determine the run order.
# By default, an empty list.
depends = ["baseos", "network"]

# Specify whether this is a one-time (true) or ongoing (false) process.
# False by default.
one-time = false

# Specify whether to restart the service if it exits dirty
# (return non-zero or core dump).
# False by default.
restart = true

# Specify whether to always restart the service when it exits.
# False by default.
restart-always = false


[process]
# Specify the command to start the service.
# The process must not demonize itself.
# This field is mandatory.
start-cmd = ["/sbin/nginx", "-g", "daemon off;"]

# Specify the command to stop the service.
# If not specified, the stop-sig will be used instead.
stop-cmd = ["/sbin/nginx", "-s", "stop"]

# Specify the signal (as int) to send to stop the service.
# The default is SIGTERM (15).
stop-sig = 15

# Specify the command to restart the service.
# If not specified, the restart-sig will be used instead.
restart-cmd = ["/sbin/nginx", "-s", "reload"]

# Specify the signal (as int) to send to restart the service.
# If both restart-cmd and restart-sig are not defined,
# stop-cmd/stop-sig followed by start-cmd will be used instead.
restart-sig = 10

# Specify the command to reload the service.
# If not specified, the reload-sig will be used instead.
reload-cmd = ["/sbin/nginx", "-s", "reload"]

# Specify the signal (as int) to send to reload the service.
# The default is SIGHUP (1).
reload-sig = 1


[system]
# Specify the user under which the start-cmd process will be called.
# This field is used only when Control is running as root.
user = "nobody"

# Specify the group under which the start-cmd process will be called.
# This field is used only when Control is running as root.
group = "nobody"

# Specify the working directory for the start-cmd process.
workdir = "/var/empty"
```

## Playground
You can download a buildroot based Linux image with Definite+Control installed. A start-qemu.sh script is provided. It opens SSH on  port 60022 and Nginx on port 60080.

[https://drive.proton.me/urls/3ZB4GSQTCG#iZCIw0suJjuH](https://drive.proton.me/urls/3ZB4GSQTCG#iZCIw0suJjuH)
