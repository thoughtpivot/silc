//! THO-121: exiting silc must stop the loop kernel; a second kernel must refuse
//! a live app.db; a clean restart must succeed.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

fn silc_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_silc"))
}

fn free_port(port: u16) {
    let _ = Command::new("bash")
        .args([
            "-c",
            &format!("lsof -ti:{port} | xargs kill -9 2>/dev/null || true"),
        ])
        .status();
    thread::sleep(Duration::from_millis(200));
}

fn process_alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

fn kernel_pids(runtime: &Path) -> Vec<u32> {
    let kernel = runtime.join("go/loop/kernel");
    let needle = kernel.to_string_lossy().into_owned();
    let out = Command::new("bash")
        .args([
            "-c",
            &format!(
                "ps -eo pid=,args= | awk -v n={} 'index($0,n){{print $1}}'",
                shell_escape(&needle)
            ),
        ])
        .output()
        .expect("ps");
    String::from_utf8_lossy(&out.stdout)
        .split_whitespace()
        .filter_map(|s| s.parse().ok())
        .collect()
}

fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn wait_for_log(path: &Path, needle: &str, timeout: Duration) -> String {
    let start = Instant::now();
    loop {
        let body = std::fs::read_to_string(path).unwrap_or_default();
        if body.contains(needle) {
            return body;
        }
        if start.elapsed() > timeout {
            return body;
        }
        thread::sleep(Duration::from_millis(200));
    }
}

fn wait_until_dead(pids: &[u32], timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if pids.iter().all(|p| !process_alive(*p)) {
            return true;
        }
        thread::sleep(Duration::from_millis(50));
    }
    pids.iter().all(|p| !process_alive(*p))
}

#[test]
fn exiting_silc_stops_loop_kernel_and_db_guard_works() {
    const HTTP_PORT: u16 = 18921;
    free_port(HTTP_PORT);

    let temp = std::env::temp_dir().join(format!(
        "silc-loop-life-{}-{}",
        std::process::id(),
        Instant::now().elapsed().as_nanos()
    ));
    std::fs::create_dir_all(&temp).unwrap();

    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/loop_pulse.silc");
    let entry = temp.join("main.silc");
    std::fs::copy(&fixture, &entry).unwrap();

    let build = Command::new(silc_bin())
        .args(["build", entry.to_str().unwrap()])
        .current_dir(&temp)
        .output()
        .expect("build");
    assert!(
        build.status.success(),
        "build failed:\n{}\n{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );

    let runtime = temp.join(".runtime/main");
    let kernel_bin = runtime.join("go/loop/kernel");
    assert!(kernel_bin.is_file(), "missing loop kernel binary");
    let db_path = runtime.join("data/app.db");
    let plan_path = runtime.join("loop/plan.json");

    // --- 1) Start silc; wait for kernel ready; SIGTERM; kernel must die. ---
    let log_path = temp.join("run1.log");
    let log = std::fs::File::create(&log_path).unwrap();
    let mut child = Command::new(silc_bin())
        .arg(entry.to_str().unwrap())
        .current_dir(&temp)
        .env("SILC_HTTP_PORT", HTTP_PORT.to_string())
        .stdout(Stdio::from(log.try_clone().unwrap()))
        .stderr(Stdio::from(log))
        .spawn()
        .expect("run silc");

    let body = wait_for_log(&log_path, "kernel ready", Duration::from_secs(180));
    assert!(
        body.contains("kernel ready"),
        "kernel never became ready:\n{body}"
    );
    let ready = Instant::now();
    let mut healthy = false;
    while ready.elapsed() < Duration::from_secs(60) {
        if let Ok(resp) = ureq::get(&format!("http://127.0.0.1:{HTTP_PORT}/health")).call() {
            let text = resp.into_string().unwrap_or_default();
            if text.contains("\"ok\"") {
                healthy = true;
                break;
            }
        }
        if let Ok(Some(status)) = child.try_wait() {
            let log = std::fs::read_to_string(&log_path).unwrap_or_default();
            panic!("silc exited early: {status}\n{log}");
        }
        thread::sleep(Duration::from_millis(200));
    }
    assert!(healthy, "ui never became healthy");

    let pids = kernel_pids(&runtime);
    assert!(
        !pids.is_empty(),
        "expected a live loop kernel process under {}",
        kernel_bin.display()
    );

    unsafe {
        libc::kill(child.id() as libc::pid_t, libc::SIGTERM);
    }
    let _ = child.wait();
    assert!(
        wait_until_dead(&pids, Duration::from_secs(5)),
        "loop kernel still running after silc SIGTERM: {pids:?}"
    );
    // Confirm no orphaned workers for this runtime remain (exclude the ps/awk probe itself).
    let leftover = Command::new("bash")
        .args([
            "-c",
            &format!(
                "pgrep -af '{}/' | grep -E '(go/worker|go/loop/kernel|typescript/worker)' | grep -v pgrep || true",
                runtime.display()
            ),
        ])
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&leftover.stdout).trim().is_empty(),
        "workers still alive after silc exit:\n{}",
        String::from_utf8_lossy(&leftover.stdout)
    );

    // --- 2) Second kernel refuses a live holder of app.db. ---
    let mut holder = Command::new(&kernel_bin)
        .env("SILC_DB_PATH", &db_path)
        .env("SILC_LOOP_PLAN", &plan_path)
        .env("SILC_LOOP_TICK_MS", "60000")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn holder kernel");
    thread::sleep(Duration::from_millis(400));
    assert!(
        process_alive(holder.id()),
        "holder kernel should still be running"
    );

    let refused = Command::new(&kernel_bin)
        .env("SILC_DB_PATH", &db_path)
        .env("SILC_LOOP_PLAN", &plan_path)
        .env("SILC_LOOP_TICK_MS", "60000")
        .output()
        .expect("spawn second kernel");
    assert!(
        !refused.status.success(),
        "second kernel should refuse a live app.db"
    );
    let err = format!(
        "{}{}",
        String::from_utf8_lossy(&refused.stdout),
        String::from_utf8_lossy(&refused.stderr)
    );
    assert!(
        err.contains("live loop kernel"),
        "expected live-kernel refuse message, got:\n{err}"
    );

    // --- 3) Clean release + restart succeeds. ---
    unsafe {
        libc::kill(holder.id() as libc::pid_t, libc::SIGTERM);
    }
    let _ = holder.wait();
    assert!(
        wait_until_dead(&[holder.id()], Duration::from_secs(3)),
        "holder should exit"
    );

    let mut restarted = Command::new(&kernel_bin)
        .env("SILC_DB_PATH", &db_path)
        .env("SILC_LOOP_PLAN", &plan_path)
        .env("SILC_LOOP_TICK_MS", "60000")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("restart kernel");
    thread::sleep(Duration::from_millis(500));
    assert!(
        process_alive(restarted.id()),
        "restart after clean exit must succeed"
    );
    unsafe {
        libc::kill(restarted.id() as libc::pid_t, libc::SIGTERM);
    }
    let _ = restarted.wait();

    // --- 4) SIGKILL of silc must not leave an orphan kernel (PDEATHSIG). ---
    let log2 = temp.join("run2.log");
    let log = std::fs::File::create(&log2).unwrap();
    let mut child2 = Command::new(silc_bin())
        .arg(entry.to_str().unwrap())
        .current_dir(&temp)
        .env("SILC_HTTP_PORT", HTTP_PORT.to_string())
        .stdout(Stdio::from(log.try_clone().unwrap()))
        .stderr(Stdio::from(log))
        .spawn()
        .expect("run silc again");
    let body2 = wait_for_log(&log2, "kernel ready", Duration::from_secs(180));
    assert!(
        body2.contains("kernel ready"),
        "second silc run never ready:\n{body2}"
    );
    let pids2 = kernel_pids(&runtime);
    assert!(!pids2.is_empty(), "expected kernel for SIGKILL case");
    unsafe {
        libc::kill(child2.id() as libc::pid_t, libc::SIGKILL);
    }
    let _ = child2.wait();
    assert!(
        wait_until_dead(&pids2, Duration::from_secs(5)),
        "loop kernel orphaned under pid 1 after silc SIGKILL: {pids2:?}"
    );

    let _ = std::fs::remove_dir_all(&temp);
}
