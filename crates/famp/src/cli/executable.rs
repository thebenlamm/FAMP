//! Resolution and validation of the FAMP executable embedded in generated config.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FampExecutable {
    path: PathBuf,
    /// The same path as UTF-8, captured at validation time so no accessor
    /// has to re-check (and silently fall back on) the UTF-8 invariant.
    utf8: String,
}

impl FampExecutable {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn utf8(&self) -> &str {
        &self.utf8
    }

    #[cfg(test)]
    pub(crate) fn validate(path: PathBuf) -> Result<Self, FampExecutableError> {
        validate_candidate(path, CandidateSource::Explicit)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FampExecutableError {
    #[error("FAMP_INSTALL_FAMP_BIN is set but empty or whitespace-only; set it to an executable FAMP file or unset it")]
    EmptyExplicit,
    #[error("{origin} FAMP executable path is not valid UTF-8: {path:?}")]
    NonUtf8 { origin: &'static str, path: PathBuf },
    // Display omits the io error itself: it is exposed as `#[source]`, and the
    // main binary walks the chain — embedding it here would print it twice.
    #[error("{origin} FAMP executable path does not exist or cannot be inspected: {}", path.display())]
    Metadata {
        origin: &'static str,
        path: PathBuf,
        #[source]
        error: std::io::Error,
    },
    #[error("{origin} FAMP executable path is not a regular file: {}", path.display())]
    NotAFile { origin: &'static str, path: PathBuf },
    #[cfg(unix)]
    #[error("{origin} FAMP executable path is not executable: {}", path.display())]
    NotExecutable { origin: &'static str, path: PathBuf },
    #[error("could not resolve the FAMP executable for generated configuration; run the installer with the `famp` binary on PATH or set FAMP_INSTALL_FAMP_BIN to its absolute path")]
    NotFound,
    #[error("could not make {origin} FAMP executable path absolute: {}", path.display())]
    Absolute {
        origin: &'static str,
        path: PathBuf,
        #[source]
        error: std::io::Error,
    },
}

pub(crate) trait FampExecutableLocator {
    fn explicit(&self) -> Option<OsString>;
    fn current_exe(&self) -> Option<PathBuf>;
    fn path_lookup(&self) -> Option<PathBuf>;
    fn current_dir(&self) -> Result<PathBuf, std::io::Error>;
}

struct ProcessLocator;

impl FampExecutableLocator for ProcessLocator {
    fn explicit(&self) -> Option<OsString> {
        std::env::var_os("FAMP_INSTALL_FAMP_BIN")
    }
    fn current_exe(&self) -> Option<PathBuf> {
        std::env::current_exe().ok()
    }
    fn path_lookup(&self) -> Option<PathBuf> {
        which::which("famp").ok()
    }
    fn current_dir(&self) -> Result<PathBuf, std::io::Error> {
        std::env::current_dir()
    }
}

pub(crate) fn resolve_for_generated_config() -> Result<FampExecutable, FampExecutableError> {
    resolve_with(&ProcessLocator)
}

pub(crate) fn resolve_with(
    locator: &impl FampExecutableLocator,
) -> Result<FampExecutable, FampExecutableError> {
    if let Some(raw) = locator.explicit() {
        if raw.to_string_lossy().trim().is_empty() {
            return Err(FampExecutableError::EmptyExplicit);
        }
        return absolute_and_validate(PathBuf::from(raw), CandidateSource::Explicit, locator);
    }
    if let Some(path) = locator.current_exe() {
        // Exact filename match only. A `file_stem()` test would also accept
        // `famp.bak` / `famp.1` / `famp.orig`, pinning a backup copy of the
        // binary into every generated config.
        if path.file_name().is_some_and(is_famp_executable_name) {
            return absolute_and_validate(path, CandidateSource::CurrentExe, locator);
        }
    }
    if let Some(path) = locator.path_lookup() {
        return absolute_and_validate(path, CandidateSource::Path, locator);
    }
    Err(FampExecutableError::NotFound)
}

/// Filename convention used to decide whether a candidate file name names the
/// FAMP executable. Parameterized (rather than `cfg!`-branched inline) so both
/// conventions are directly testable on any host.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum NameConvention {
    /// Exactly `famp`, byte-for-byte. No extension is accepted.
    Unix,
    /// Exactly `famp.exe`, compared ASCII-case-insensitively (Windows file
    /// names are case-insensitive, and only `.exe` is an executable image).
    Windows,
}

/// The convention this build's filesystem actually uses.
const HOST_NAME_CONVENTION: NameConvention = if cfg!(windows) {
    NameConvention::Windows
} else {
    NameConvention::Unix
};

/// True when `file_name` is an accepted FAMP executable file name under
/// `convention`.
///
/// Deliberately strict: `famp.bak`, `famp.1`, `famp.exe.bak`, `famp-old` and
/// any test-harness binary name are rejected. Only an exact platform match
/// lets the *currently running* executable be pinned into generated config.
pub(crate) fn is_famp_executable_name_for(file_name: &OsStr, convention: NameConvention) -> bool {
    match convention {
        NameConvention::Unix => file_name == OsStr::new("famp"),
        NameConvention::Windows => file_name
            .to_str()
            .is_some_and(|name| name.eq_ignore_ascii_case("famp.exe")),
    }
}

/// `is_famp_executable_name_for` under this host's convention.
pub(crate) fn is_famp_executable_name(file_name: &OsStr) -> bool {
    is_famp_executable_name_for(file_name, HOST_NAME_CONVENTION)
}

#[derive(Clone, Copy)]
enum CandidateSource {
    Explicit,
    CurrentExe,
    Path,
}

impl CandidateSource {
    const fn label(self) -> &'static str {
        match self {
            Self::Explicit => "explicit",
            Self::CurrentExe => "current",
            Self::Path => "PATH-discovered",
        }
    }
}

fn absolute_and_validate(
    path: PathBuf,
    source: CandidateSource,
    locator: &impl FampExecutableLocator,
) -> Result<FampExecutable, FampExecutableError> {
    let absolute = if path.is_absolute() {
        path
    } else {
        locator
            .current_dir()
            .map(|cwd| cwd.join(&path))
            .map_err(|error| FampExecutableError::Absolute {
                origin: source.label(),
                path,
                error,
            })?
    };
    validate_candidate(absolute, source)
}

fn validate_candidate(
    path: PathBuf,
    source: CandidateSource,
) -> Result<FampExecutable, FampExecutableError> {
    let Some(utf8) = path.to_str().map(str::to_string) else {
        return Err(FampExecutableError::NonUtf8 {
            origin: source.label(),
            path,
        });
    };
    let metadata = std::fs::metadata(&path).map_err(|error| FampExecutableError::Metadata {
        origin: source.label(),
        path: path.clone(),
        error,
    })?;
    if !metadata.is_file() {
        return Err(FampExecutableError::NotAFile {
            origin: source.label(),
            path,
        });
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return Err(FampExecutableError::NotExecutable {
                origin: source.label(),
                path,
            });
        }
    }
    Ok(FampExecutable { path, utf8 })
}

/// Flatten an error and its `source()` chain into a single line.
///
/// For surfaces that can only carry a `String` (the daemon lifecycle errors),
/// so the underlying cause is not lost when the typed error cannot travel.
pub(crate) fn flatten_error_chain(error: &dyn std::error::Error) -> String {
    let mut rendered = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        use std::fmt::Write as _;
        let _ = write!(rendered, ": {cause}");
        source = cause.source();
    }
    rendered
}

/// Quote arbitrary UTF-8 text as one POSIX shell word.
///
/// Public so integration tests can render the shipped hook assets exactly the
/// way `await_hook::install_shim` / `hook_runner::install_shim` do.
#[must_use]
pub fn posix_shell_literal(raw: &str) -> String {
    format!("'{}'", raw.replace('\'', "'\\''"))
}

/// Test-only helpers shared by the installer "resolution fails before any
/// mutation" suites (`cli::install::{claude_code,codex,grok}`,
/// `cli::daemon::install`).
#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub(crate) mod test_support {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    /// A `FAMP_INSTALL_FAMP_BIN` value that cannot resolve on any host:
    /// nothing exists at this path, so the resolver must fail closed
    /// (`FampExecutableError::Metadata`) rather than fall back.
    pub const MISSING_FAMP_BIN: &str = "/famp-resolver-must-fail/nonexistent/famp";

    /// One filesystem entry: `None` marks a directory, `Some(bytes)` a file's
    /// exact contents. Symlinks are read through (installers never create
    /// them, so a symlink appearing at all would be a change).
    pub type TreeSnapshot = BTreeMap<PathBuf, Option<Vec<u8>>>;

    /// Recursively snapshot every directory and file under `root`.
    /// A missing `root` snapshots as an empty map.
    pub fn snapshot_tree(root: &Path) -> TreeSnapshot {
        let mut out = TreeSnapshot::new();
        let Ok(entries) = std::fs::read_dir(root) else {
            return out;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                out.insert(path.clone(), None);
                out.extend(snapshot_tree(&path));
            } else {
                out.insert(path.clone(), Some(std::fs::read(&path).unwrap_or_default()));
            }
        }
        out
    }

    /// Assert the tree under `root` is byte-for-byte the `before` snapshot:
    /// no file rewritten, none removed, and no new file or directory created
    /// (backups, hook shims, service files and `~/.famp/` all included).
    pub fn assert_tree_unchanged(root: &Path, before: &TreeSnapshot, context: &str) {
        let after = snapshot_tree(root);
        let added: Vec<&PathBuf> = after.keys().filter(|k| !before.contains_key(*k)).collect();
        assert!(
            added.is_empty(),
            "{context}: resolution failure must not create anything, but got {added:#?}"
        );
        let removed: Vec<&PathBuf> = before.keys().filter(|k| !after.contains_key(*k)).collect();
        assert!(
            removed.is_empty(),
            "{context}: resolution failure must not remove anything, but lost {removed:#?}"
        );
        for (path, bytes) in before {
            assert_eq!(
                after.get(path),
                Some(bytes),
                "{context}: {} changed on disk",
                path.display()
            );
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    struct FakeLocator {
        explicit: Option<OsString>,
        current: Option<PathBuf>,
        path: Option<PathBuf>,
        cwd: PathBuf,
        /// When true, `current_dir()` fails the way a deleted working
        /// directory makes `std::env::current_dir()` fail.
        cwd_fails: bool,
    }

    impl FampExecutableLocator for FakeLocator {
        fn explicit(&self) -> Option<OsString> {
            self.explicit.clone()
        }
        fn current_exe(&self) -> Option<PathBuf> {
            self.current.clone()
        }
        fn path_lookup(&self) -> Option<PathBuf> {
            self.path.clone()
        }
        fn current_dir(&self) -> Result<PathBuf, std::io::Error> {
            if self.cwd_fails {
                return Err(std::io::Error::from(std::io::ErrorKind::NotFound));
            }
            Ok(self.cwd.clone())
        }
    }

    fn executable(path: &Path) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, "#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    fn locator(cwd: &Path) -> FakeLocator {
        FakeLocator {
            explicit: None,
            current: None,
            path: None,
            cwd: cwd.to_path_buf(),
            cwd_fails: false,
        }
    }

    #[test]
    fn precedence_and_candidate_selection_are_deterministic() {
        let dir = tempfile::tempdir().unwrap();
        let explicit = dir.path().join("explicit famp");
        let current = dir.path().join("famp");
        let path = dir.path().join("path-famp");
        for candidate in [&explicit, &current, &path] {
            executable(candidate);
        }
        let mut fake = locator(dir.path());
        fake.explicit = Some(explicit.clone().into_os_string());
        fake.current = Some(current.clone());
        fake.path = Some(path.clone());
        assert_eq!(resolve_with(&fake).unwrap().path(), explicit);

        fake.explicit = None;
        assert_eq!(resolve_with(&fake).unwrap().path(), current);
        fake.current = Some(dir.path().join("test-harness"));
        assert_eq!(resolve_with(&fake).unwrap().path(), path);
        fake.path = None;
        assert!(matches!(
            resolve_with(&fake),
            Err(FampExecutableError::NotFound)
        ));
    }

    #[test]
    fn invalid_explicit_never_falls_back() {
        let dir = tempfile::tempdir().unwrap();
        let fallback = dir.path().join("famp");
        executable(&fallback);
        let mut fake = locator(dir.path());
        fake.path = Some(fallback);
        fake.explicit = Some(OsString::from("  \t"));
        assert!(matches!(
            resolve_with(&fake),
            Err(FampExecutableError::EmptyExplicit)
        ));
        fake.explicit = Some(OsString::from("missing"));
        assert!(matches!(
            resolve_with(&fake),
            Err(FampExecutableError::Metadata { .. })
        ));
    }

    #[test]
    fn relative_space_and_cargo_style_paths_become_absolute() {
        let dir = tempfile::tempdir().unwrap();
        let cargo = dir.path().join(".cargo/bin/famp with space");
        executable(&cargo);
        let mut fake = locator(dir.path());
        fake.explicit = Some(OsString::from(".cargo/bin/famp with space"));
        assert_eq!(resolve_with(&fake).unwrap().path(), cargo);
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_preserved_and_invalid_file_types_fail() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("real");
        let link = dir.path().join("famp link");
        executable(&target);
        symlink(&target, &link).unwrap();
        let mut fake = locator(dir.path());
        fake.explicit = Some(link.clone().into_os_string());
        assert_eq!(resolve_with(&fake).unwrap().path(), link);

        let broken = dir.path().join("broken");
        symlink(dir.path().join("absent"), &broken).unwrap();
        fake.explicit = Some(broken.into_os_string());
        assert!(matches!(
            resolve_with(&fake),
            Err(FampExecutableError::Metadata { .. })
        ));
        fake.explicit = Some(dir.path().to_path_buf().into_os_string());
        assert!(matches!(
            resolve_with(&fake),
            Err(FampExecutableError::NotAFile { .. })
        ));
        let plain = dir.path().join("plain");
        std::fs::write(&plain, "x").unwrap();
        std::fs::set_permissions(&plain, std::fs::Permissions::from_mode(0o644)).unwrap();
        fake.explicit = Some(plain.into_os_string());
        assert!(matches!(
            resolve_with(&fake),
            Err(FampExecutableError::NotExecutable { .. })
        ));
    }

    /// A non-UTF-8 candidate path is rejected before the filesystem is
    /// consulted at all — `NonUtf8`, never `Metadata`.
    ///
    /// The file is deliberately NOT created: macOS rejects non-UTF-8 file
    /// names outright (`EILSEQ`, "Illegal byte sequence"), so staging one is
    /// impossible there. That is fine — asserting `NonUtf8` against a path
    /// that does not exist is the stronger claim, since it proves the UTF-8
    /// check runs first rather than being masked by a `Metadata` error.
    #[cfg(unix)]
    #[test]
    fn non_utf8_path_fails() {
        use std::os::unix::ffi::OsStringExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(OsString::from_vec(vec![b'f', 0x80]));
        let mut fake = locator(dir.path());
        fake.explicit = Some(path.into_os_string());
        assert!(matches!(
            resolve_with(&fake),
            Err(FampExecutableError::NonUtf8 { .. })
        ));
    }

    #[test]
    fn posix_literal_quotes_spaces_apostrophes_and_shell_metacharacters() {
        assert_eq!(posix_shell_literal("/tmp/a b'c;$x"), "'/tmp/a b'\\''c;$x'");
    }

    /// M1: only an exact platform file name may pin the *running* executable
    /// into generated config. `file_stem()`-style matching would accept a
    /// backup copy (`famp.bak`) or a man page-ish sibling (`famp.1`).
    #[test]
    fn only_exact_platform_file_names_identify_the_famp_executable() {
        use NameConvention::{Unix, Windows};
        let accepted: &[(&str, NameConvention)] = &[
            ("famp", Unix),
            ("famp.exe", Windows),
            ("FAMP.EXE", Windows),
            ("Famp.Exe", Windows),
        ];
        for (name, convention) in accepted {
            assert!(
                is_famp_executable_name_for(OsStr::new(name), *convention),
                "{name} must be accepted under {convention:?}"
            );
        }

        let rejected = [
            "famp.bak",
            "famp.1",
            "famp.exe.bak",
            "famp.old",
            "famp-old",
            "fampx",
            "cargo-famp",
            "famp ",
            " famp",
            "",
            // The test harness binary that actually runs this module.
            "executable-3f2a1c0d9e8b7a65",
        ];
        for name in rejected {
            for convention in [Unix, Windows] {
                assert!(
                    !is_famp_executable_name_for(OsStr::new(name), convention),
                    "{name:?} must be rejected under {convention:?}"
                );
            }
        }
        // Cross-convention strictness: a Windows image name is not a Unix
        // executable name and vice versa.
        assert!(!is_famp_executable_name_for(OsStr::new("famp.exe"), Unix));
        assert!(!is_famp_executable_name_for(OsStr::new("famp"), Windows));

        // The real running test binary must never be mistaken for `famp`.
        let current = std::env::current_exe().unwrap();
        assert!(
            !is_famp_executable_name(current.file_name().unwrap()),
            "the test harness binary {} must not be accepted",
            current.display()
        );
    }

    /// A `current_exe` whose name only *stems* from `famp` must be skipped so
    /// resolution falls through to PATH rather than pinning the backup file.
    #[test]
    fn current_exe_with_extension_is_not_a_candidate() {
        let dir = tempfile::tempdir().unwrap();
        let backup = dir.path().join("famp.bak");
        let on_path = dir.path().join("bin/famp");
        executable(&backup);
        executable(&on_path);
        let mut fake = locator(dir.path());
        fake.current = Some(backup);
        fake.path = Some(on_path.clone());
        assert_eq!(resolve_with(&fake).unwrap().path(), on_path);

        // …and with nothing on PATH it is a hard failure, not a silent pin.
        fake.path = None;
        assert!(matches!(
            resolve_with(&fake),
            Err(FampExecutableError::NotFound)
        ));

        // The exact name is still accepted.
        fake.current = Some(on_path.clone());
        assert_eq!(resolve_with(&fake).unwrap().path(), on_path);
    }

    /// Leading/trailing spaces are real path characters, not noise to trim:
    /// the resolver must select the file the operator actually named.
    #[cfg(unix)]
    #[test]
    fn leading_and_trailing_space_file_names_are_preserved() {
        let dir = tempfile::tempdir().unwrap();
        for name in [" famp", "famp "] {
            let candidate = dir.path().join(name);
            executable(&candidate);
            let mut fake = locator(dir.path());
            fake.explicit = Some(candidate.clone().into_os_string());
            let resolved = resolve_with(&fake).unwrap();
            assert_eq!(resolved.path(), candidate);
            assert_eq!(resolved.utf8(), candidate.to_str().unwrap());
            assert!(resolved.utf8().ends_with(name));
        }
    }

    /// A non-UTF-8 `FAMP_INSTALL_FAMP_BIN` value fails closed: generated
    /// config is UTF-8 JSON/TOML/XML, so there is nothing safe to emit.
    #[cfg(unix)]
    #[test]
    fn non_utf8_explicit_env_value_fails_without_fallback() {
        use std::os::unix::ffi::OsStringExt;
        let dir = tempfile::tempdir().unwrap();
        let fallback = dir.path().join("famp");
        executable(&fallback);
        let mut fake = locator(dir.path());
        fake.path = Some(fallback);

        // Relative (cwd-joined) and absolute non-UTF-8 values both fail.
        fake.explicit = Some(OsString::from_vec(vec![b'r', 0xff, b'x']));
        assert!(matches!(
            resolve_with(&fake),
            Err(FampExecutableError::NonUtf8 { .. })
        ));
        let mut absolute = dir.path().to_path_buf().into_os_string().into_vec();
        absolute.extend_from_slice(&[b'/', 0xff]);
        fake.explicit = Some(OsString::from_vec(absolute));
        assert!(matches!(
            resolve_with(&fake),
            Err(FampExecutableError::NonUtf8 { .. })
        ));
    }

    /// A relative explicit path needs the working directory to become
    /// absolute. When that lookup fails, resolution fails — it must not fall
    /// through to PATH or embed a relative path into generated config.
    #[test]
    fn current_dir_failure_on_relative_explicit_path_is_fatal() {
        let dir = tempfile::tempdir().unwrap();
        let fallback = dir.path().join("famp");
        executable(&fallback);
        let mut fake = locator(dir.path());
        fake.path = Some(fallback);
        fake.cwd_fails = true;
        fake.explicit = Some(OsString::from("relative/famp"));
        assert!(matches!(
            resolve_with(&fake),
            Err(FampExecutableError::Absolute { .. })
        ));

        // An absolute explicit path never consults the working directory.
        let absolute = dir.path().join("elsewhere/famp");
        executable(&absolute);
        fake.explicit = Some(absolute.clone().into_os_string());
        assert_eq!(resolve_with(&fake).unwrap().path(), absolute);
    }

    /// A `~/.cargo/bin/famp`-shaped path is an ordinary selection like any
    /// other — supported when it is the real executable, never invented as a
    /// fallback.
    #[test]
    fn cargo_shaped_path_is_preserved_when_actually_selected() {
        let dir = tempfile::tempdir().unwrap();
        let cargo = dir.path().join(".cargo/bin/famp");
        executable(&cargo);
        let mut fake = locator(dir.path());

        fake.explicit = Some(cargo.clone().into_os_string());
        assert_eq!(resolve_with(&fake).unwrap().path(), cargo);
        fake.explicit = None;
        fake.current = Some(cargo.clone());
        assert_eq!(resolve_with(&fake).unwrap().path(), cargo);
        fake.current = None;
        fake.path = Some(cargo.clone());
        assert_eq!(resolve_with(&fake).unwrap().utf8(), cargo.to_str().unwrap());

        // But it is never conjured: with no candidate at all, resolution fails.
        fake.path = None;
        assert!(matches!(
            resolve_with(&fake),
            Err(FampExecutableError::NotFound)
        ));
    }
}
