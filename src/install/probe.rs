//! Read-only execution-alias check. The existing Core probe opens no project or Host.
use crate::agent::{DescriptorV1, ProbeReplyV1, PROBE_ARG, SCHEMA_VERSION};
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const LIMIT: usize = 16 * 1024;
const TIMEOUT: Duration = Duration::from_secs(5);

pub(super) fn check_alias(alias: &Path) -> Result<(), String> {
    let check = || {
        let mut random = [0_u8; 16];
        getrandom::fill(&mut random).map_err(|err| format!("cannot generate probe challenge: {err}"))?;
        let challenge = super::hex(&random);
        let mut command = Command::new(alias);
        command.args([PROBE_ARG, &challenge]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let bytes = run_probe(&mut command, TIMEOUT)?;
        validate_reply(&bytes, &challenge)
    };
    check().map_err(|why| format!(
        "app execution alias {} failed: {why}. Check Windows Settings > Apps > App execution aliases for gil.exe; enable it or repair/reinstall the GIL Store app",
        alias.display()
    ))
}

fn validate_reply(bytes: &[u8], challenge: &str) -> Result<(), String> {
    let reply: ProbeReplyV1 = serde_json::from_slice(bytes)
        .map_err(|_| "alias did not return a valid GIL Core probe".to_owned())?;
    if reply.schema_version != SCHEMA_VERSION || reply.challenge != challenge {
        return Err("alias returned a different probe schema or challenge".into());
    }
    if reply.core != DescriptorV1::current() {
        return Err("alias targets a different Core version, platform or capability profile".into());
    }
    Ok(())
}

fn run_probe(command: &mut Command, timeout: Duration) -> Result<Vec<u8>, String> {
    let mut child = command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null())
        .spawn().map_err(|err| format!("cannot execute alias: {err}"))?;
    let stdout = child.stdout.take().expect("piped stdout");
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    // Drain concurrently: a bad target must not block on a full pipe or allocate unbounded RAM.
    // The caller also bounds EOF waiting, including a pipe inherited by another process.
    let reader = std::thread::Builder::new().name("gil-alias-probe".into()).spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout.take((LIMIT + 1) as u64).read_to_end(&mut bytes)
            .map_err(|err| format!("cannot read alias probe: {err}"))
            .and_then(|_| if bytes.len() > LIMIT { Err("alias probe exceeded output limit".into()) } else { Ok(bytes) });
        let _ = send.send(result);
    });
    let result = if let Err(err) = reader {
        Err(format!("cannot start alias probe reader: {err}"))
    } else {
        let started = Instant::now();
        let mut reply = None;
        loop {
            match receive.try_recv() {
                Ok(Ok(bytes)) => reply = Some(bytes),
                Ok(Err(why)) => break Err(why),
                Err(std::sync::mpsc::TryRecvError::Disconnected) if reply.is_none() => break Err("alias probe reader stopped".into()),
                _ => {}
            }
            match child.try_wait() {
                Ok(Some(status)) if !status.success() => break Err(format!("alias exited with {status}")),
                Ok(Some(_)) if reply.is_some() => break Ok(reply.take().unwrap()),
                Err(err) => break Err(format!("cannot wait for alias: {err}")),
                _ => {}
            }
            if started.elapsed() >= timeout { break Err("alias probe timed out".into()); }
            std::thread::sleep(Duration::from_millis(10));
        }
    };
    if result.is_err() { let _ = child.kill(); }
    let _ = child.wait();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reply_requires_this_core_and_the_fresh_challenge() {
        let challenge = "fresh_challenge_0001";
        let mut reply = ProbeReplyV1 { schema_version: SCHEMA_VERSION, challenge: challenge.into(), core: DescriptorV1::current() };
        let check = |reply: &ProbeReplyV1| validate_reply(&serde_json::to_vec(reply).unwrap(), challenge);
        assert!(check(&reply).is_ok());
        reply.challenge = "old_challenge_00001".into();
        assert!(check(&reply).is_err());
        reply.challenge = challenge.into();
        reply.core.core_version = "different-version".into();
        assert!(check(&reply).is_err());
        reply.core = DescriptorV1::current();
        reply.core.product = "another-app".into();
        assert!(check(&reply).is_err());
        assert!(validate_reply(b"not JSON", challenge).is_err());
    }

    #[test]
    fn a_missing_alias_is_an_actionable_error() {
        let root = super::super::tests::scratch("missing-alias");
        let error = check_alias(&root.join("gil.exe")).unwrap_err();
        assert!(error.contains("cannot execute alias"));
        assert!(error.contains("App execution aliases"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn process_probe_bounds_output_time_and_exit_status() {
        let run = |script: &str| run_probe(Command::new("/bin/sh").args(["-c", script]), Duration::from_millis(150));
        assert_eq!(run("printf small").unwrap(), b"small");
        assert!(run("exit 7").unwrap_err().contains("exited"));
        assert!(run("while :; do printf '0123456789'; done").unwrap_err().contains("output limit"));
        assert!(run("while :; do :; done").unwrap_err().contains("timed out"));
    }

    #[cfg(windows)]
    #[test]
    fn process_probe_bounds_output_time_and_exit_status() {
        let run = |script: &str, timeout| run_probe(
            Command::new("powershell.exe").args(["-NoProfile", "-NonInteractive", "-Command", script]), timeout
        );
        let normal = Duration::from_secs(10);
        assert_eq!(run("[Console]::Write('small')", normal).unwrap(), b"small");
        assert!(run("exit 7", normal).unwrap_err().contains("exited"));
        assert!(run("[Console]::Write(('x' * 20000))", normal).unwrap_err().contains("output limit"));
        assert!(run("while ($true) {}", Duration::from_millis(150)).unwrap_err().contains("timed out"));
    }
}
