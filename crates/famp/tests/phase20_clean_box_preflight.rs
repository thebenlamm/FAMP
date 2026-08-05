//! Falsification suite for the read-only Phase 20 clean-host preflight.

#![allow(unused_crate_dependencies)]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

fn script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("scripts/phase20-clean-box-preflight.sh")
}

fn write_executable(path: &Path, body: &str) {
    fs::write(path, body).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
}

fn snapshot(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn walk(base: &Path, at: &Path, out: &mut Vec<(PathBuf, Vec<u8>)>) {
        let mut entries = fs::read_dir(at)
            .unwrap()
            .map(|e| e.unwrap())
            .collect::<Vec<_>>();
        entries.sort_by_key(std::fs::DirEntry::path);
        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                walk(base, &path, out);
            } else {
                out.push((
                    path.strip_prefix(base).unwrap().to_path_buf(),
                    fs::read(path).unwrap(),
                ));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out
}

struct Fixture {
    _tmp: TempDir,
    home: PathBuf,
    bin: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("secret-home");
        let bin = tmp.path().join("bin");
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&bin).unwrap();
        for (name, body) in [
            (
                "uname",
                "#!/bin/sh\n[ \"$1\" = -s ] && echo Linux || echo x86_64\n",
            ),
            ("date", "#!/bin/sh\necho 2030-01-02T03:04:05Z\n"),
            ("systemctl", "#!/bin/sh\nexit 1\n"),
        ] {
            write_executable(&bin.join(name), body);
        }
        Self {
            _tmp: tmp,
            home,
            bin,
        }
    }

    fn command(&self) -> Command {
        let mut cmd = Command::new("/bin/sh");
        cmd.arg(script())
            .env_clear()
            .env("HOME", &self.home)
            .env("PATH", &self.bin)
            .env("PHASE20_SERVICE_CHECK", "systemctl");
        cmd
    }

    fn add_binary(&self, name: &str) {
        write_executable(&self.bin.join(name), "#!/bin/sh\nexit 0\n");
    }
}

fn combined(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn pristine_control_is_read_only_redacted_and_non_vacuous() {
    let fixture = Fixture::new();
    let before = snapshot(&fixture.home);
    let output = fixture.command().output().unwrap();
    let after = snapshot(&fixture.home);
    let text = combined(&output);
    assert!(output.status.success(), "{text}");
    for anchor in [
        "OS: Linux",
        "ARCH: x86_64",
        "UTC: 2030-01-02T03:04:05Z",
        "HOME: <HOME>",
        "CLEAN HOST: PASS",
    ] {
        assert!(text.contains(anchor), "missing {anchor}: {text}");
    }
    assert!(!text.contains(fixture.home.to_string_lossy().as_ref()));
    assert_eq!(before, after, "preflight mutated controlled HOME");
}

#[test]
fn every_contamination_class_fails_with_a_named_diagnostic() {
    for (name, diagnostic) in [
        ("rustc", "CONTAMINATION: rustc"),
        ("cargo", "CONTAMINATION: cargo"),
        ("famp", "CONTAMINATION: famp"),
        ("famp-gateway", "CONTAMINATION: famp-gateway"),
    ] {
        let fixture = Fixture::new();
        fixture.add_binary(name);
        let output = fixture.command().output().unwrap();
        assert!(!output.status.success());
        assert!(combined(&output).contains(diagnostic));
    }

    let fixture = Fixture::new();
    let output = fixture
        .command()
        .env("FAMP_HOME", "/redacted/custom")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(combined(&output).contains("CONTAMINATION: FAMP_HOME"));

    for (relative, diagnostic) in [
        (".famp/state.json", "CONTAMINATION: default FAMP state"),
        (".famp/bus.sock", "CONTAMINATION: broker/socket"),
    ] {
        let fixture = Fixture::new();
        let path = fixture.home.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"present").unwrap();
        let output = fixture.command().output().unwrap();
        assert!(!output.status.success());
        assert!(combined(&output).contains(diagnostic));
    }

    let fixture = Fixture::new();
    write_executable(&fixture.bin.join("systemctl"), "#!/bin/sh\nexit 0\n");
    let output = fixture.command().output().unwrap();
    assert!(!output.status.success());
    assert!(combined(&output).contains("CONTAMINATION: service"));
}

#[test]
fn unsupported_platform_fails_closed() {
    let fixture = Fixture::new();
    write_executable(
        &fixture.bin.join("uname"),
        "#!/bin/sh\n[ \"$1\" = -s ] && echo Plan9 || echo mips\n",
    );
    let output = fixture.command().output().unwrap();
    assert!(!output.status.success());
    assert!(combined(&output).contains("UNSUPPORTED PLATFORM"));
}
