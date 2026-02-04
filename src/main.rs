use clap::Parser;
use humantime;
use std::{env, thread, time, process};

const VERSION: &'static str = concat!("v", env!("CARGO_PKG_VERSION"), "-alpha.01");
const ABOUT: &'static str = "wireguard interface restarter\n$ git clone https://github.com/zorael/wg_restarter";

#[derive(Parser)]
#[command(name = "wg_restarter")]
#[command(author = "jr <zorael@protonmail.com>")]
#[command(version = VERSION)]
#[command(about = ABOUT)]
#[command(arg_required_else_help = true)]
struct Cli {
    /// Handshake timeout in seconds
    #[arg(short = 't', long, value_parser = humantime::parse_duration, default_value = "10m")]
    timeout: time::Duration,

    /// Loop interval in seconds
    #[arg(short = 'L', long, value_parser = humantime::parse_duration, default_value = "60s")]
    loop_interval: time::Duration,

    /// Retry interval after unit restart in seconds
    #[arg(short = 'R', long, value_parser = humantime::parse_duration, default_value = "30s")]
    retry_after_unit_restart: time::Duration,

    /// WireGuard interface to monitor
    interface: Option<String>,
}

/// Parse the first peer's latest-handshake timestamp from `wg show` output.
fn first_peer_handshake_ts(output: &str) -> Option<u64> {
    // Handshakes are in the tab-separated format "HASH\t1234567890\n"
    let line = output.lines().next()?;  // first peer only
    let (_, post) = line.split_once('\t')?;

    post
        .trim()
        .parse()
        .ok()
}

/// Convert a UNIX timestamp (seconds since epoch) to SystemTime.
fn unix_ts_to_system_time(ts: u64) -> time::SystemTime {
    time::UNIX_EPOCH + time::Duration::from_secs(ts)
}

/// Check if a systemd unit is active.
fn get_systemd_unit_is_active(unit_name: &str) -> Result<bool, String> {
    match process::Command::new("systemctl")
        .args(["is-active", "-q", unit_name])
        .status()
    {
        Ok(status) if status.success() => Ok(true),
        Ok(_) => Ok(false),
        Err(e) => Err(format!("failed to run `systemctl`: {e}"))
    }
}

/// Main program entry point.
fn main() -> process::ExitCode {
    let cli = Cli::parse();

    let interface = match cli.interface.as_deref().map(str::trim) {
        Some(s) if s.is_empty() => {
            eprintln!("interface name cannot be empty; exiting ...");
            return process::ExitCode::FAILURE;
        },
        Some(s) => s,
        None => unreachable!(),  // should not happen due to clap's arg_required_else_help
    };

    let unit_name = format!("wg-quick@{interface}.service");

    match get_systemd_unit_is_active(&unit_name) {
        Ok(true) => {},
        Ok(false) => {
            eprintln!("systemd service `{unit_name}` is not active; exiting ...");
            return process::ExitCode::FAILURE;
        },
        Err(e) => {
            eprintln!("failed to run `systemctl is-active`: {e}");
            return process::ExitCode::FAILURE;
        }
    };

    // Everything looks good
    println!("monitoring wireguard interface `{interface}` with systemd unit `{unit_name}` ...");

    // Main loop start
    loop {
        // Get latest-handshakes output from `wg show`
        let wg_show = match process::Command::new("wg")
            .args(["show", &interface, "latest-handshakes"])
            .output()
        {
            Ok(output) => output,
            Err(e) => {
                eprintln!("failed to run `wg show`: {e}");
                thread::sleep(cli.loop_interval);
                continue;
            }
        };

        if !wg_show.status.success() {
            eprintln!("`wg show` returned {}: {}",
                wg_show.status.code().expect("`wg show` status code error"),
                String::from_utf8_lossy(&wg_show.stderr).trim());
            thread::sleep(cli.loop_interval);
            continue;
        }

        let stdout = String::from_utf8_lossy(&wg_show.stdout);  // no need to .trim()

        let timestamp = match first_peer_handshake_ts(&stdout) {
            Some(v) => v,
            None => {
                eprintln!("unexpected `wg show latest-handshakes` output:\n{stdout}");
                thread::sleep(cli.loop_interval);
                continue;
            }
        };

        if timestamp == 0 {
            eprintln!("no handshake recorded yet; waiting ...");
            thread::sleep(cli.loop_interval);
            continue;
        }

        let last = unix_ts_to_system_time(timestamp);
        let elapsed = time::SystemTime::now()
            .duration_since(last)
            .unwrap_or_default();

        if elapsed <= cli.timeout {
            thread::sleep(cli.loop_interval);
            continue;
        }

        eprintln!("handshake timeout; {}s > {}s. restarting service ...", elapsed.as_secs(), cli.timeout.as_secs());
        eprintln!("--> systemctl restart {unit_name}");

        let systemctl_restart = match process::Command::new("systemctl")
            .args(["restart", &unit_name])
            .status()
        {
            Ok(s) => s,
            Err(e) => {
                eprintln!("failed to execute systemctl restart: {e}");
                thread::sleep(cli.loop_interval);
                continue;
            }
        };

        if !systemctl_restart.success() {
            eprintln!("restart failed with status {}", systemctl_restart.code().unwrap_or(-1));
        }

        thread::sleep(cli.retry_after_unit_restart);
    }
}
