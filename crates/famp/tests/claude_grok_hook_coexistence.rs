//! Claude and Grok share one await-hook *source* asset but own two separate
//! *installed* files, each pinned to the executable that installed it.
//!
//! M3 of the installation-foundation review: the shared
//! `crates/famp/assets/famp-await.sh` template is rendered per install, so
//! installing Claude with binary A and Grok with binary B must leave two
//! independently pinned shims — and uninstalling one integration must not
//! disturb the other's shim.

#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used, unused_crate_dependencies)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Stage an executable file that the resolver will accept.
fn stage_executable(path: &Path) -> PathBuf {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, "#!/bin/sh\nexit 0\n").unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path.to_path_buf()
}

/// Run `test` with `FAMP_INSTALL_FAMP_BIN` pinned at `bin`.
/// WR-06: `temp_env` serializes this process-global mutation.
fn with_pinned_famp_bin<T>(bin: &Path, test: impl FnOnce() -> T) -> T {
    temp_env::with_var("FAMP_INSTALL_FAMP_BIN", Some(bin.as_os_str()), test)
}

/// The `FAMP_BIN='…'` assignment a rendered hook must carry.
fn pin_line(bin: &Path) -> String {
    format!(
        "FAMP_BIN={}",
        famp::cli::executable::posix_shell_literal(bin.to_str().unwrap())
    )
}

fn install_claude(home: &Path, bin: &Path) {
    with_pinned_famp_bin(bin, || {
        let mut out = Vec::<u8>::new();
        let mut err = Vec::<u8>::new();
        famp::cli::install::claude_code::run_at(home, &mut out, &mut err)
            .expect("install-claude-code");
    });
}

fn install_grok(home: &Path, bin: &Path) {
    with_pinned_famp_bin(bin, || {
        let mut out = Vec::<u8>::new();
        let mut err = Vec::<u8>::new();
        famp::cli::install::grok::run_at(home, &mut out, &mut err).expect("install-grok");
    });
}

fn setup(home: &Path) -> (PathBuf, PathBuf) {
    let bin_a = stage_executable(&home.join("bin-a/famp"));
    let bin_b = stage_executable(&home.join("bin-b/famp"));
    install_claude(home, &bin_a);
    install_grok(home, &bin_b);

    let claude_hook = std::fs::read_to_string(home.join(".claude/hooks/famp-await.sh")).unwrap();
    let grok_hook = std::fs::read_to_string(home.join(".grok/hooks/famp-await.sh")).unwrap();
    assert!(
        claude_hook.contains(&pin_line(&bin_a)),
        "Claude's installed hook must pin executable A"
    );
    assert!(
        !claude_hook.contains(&pin_line(&bin_b)),
        "Claude's installed hook must not carry Grok's executable"
    );
    assert!(
        grok_hook.contains(&pin_line(&bin_b)),
        "Grok's installed hook must pin executable B"
    );
    assert!(
        !grok_hook.contains(&pin_line(&bin_a)),
        "Grok's installed hook must not carry Claude's executable"
    );
    (bin_a, bin_b)
}

/// Uninstalling Claude removes Claude's shim and the shared `~/.famp`
/// hook-runner, and leaves Grok's independently pinned shim untouched.
#[test]
fn uninstall_claude_leaves_grok_hook_pinned_to_its_own_executable() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path();
    let (_bin_a, bin_b) = setup(home);

    let grok_before = std::fs::read_to_string(home.join(".grok/hooks/famp-await.sh")).unwrap();

    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    famp::cli::uninstall::claude_code::run_at(home, &mut out, &mut err)
        .expect("uninstall-claude-code");

    assert!(
        !home.join(".claude/hooks/famp-await.sh").exists(),
        "uninstall-claude-code must remove Claude's own shim"
    );
    let grok_after = std::fs::read_to_string(home.join(".grok/hooks/famp-await.sh")).unwrap();
    assert_eq!(
        grok_after, grok_before,
        "uninstall-claude-code must not touch Grok's installed hook"
    );
    assert!(
        grok_after.contains(&pin_line(&bin_b)),
        "Grok's hook must still pin its own executable after Claude is removed"
    );
    assert!(home.join(".grok/hooks/famp-listen-stop.json").exists());
}

/// Reverse direction: uninstalling Grok leaves Claude's shim intact.
#[test]
fn uninstall_grok_leaves_claude_hook_pinned_to_its_own_executable() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path();
    let (bin_a, _bin_b) = setup(home);

    let claude_before = std::fs::read_to_string(home.join(".claude/hooks/famp-await.sh")).unwrap();

    let mut out = Vec::<u8>::new();
    let mut err = Vec::<u8>::new();
    famp::cli::uninstall::grok::run_at(home, &mut out, &mut err).expect("uninstall-grok");

    assert!(
        !home.join(".grok/hooks/famp-await.sh").exists(),
        "uninstall-grok must remove Grok's own shim"
    );
    let claude_after = std::fs::read_to_string(home.join(".claude/hooks/famp-await.sh")).unwrap();
    assert_eq!(
        claude_after, claude_before,
        "uninstall-grok must not touch Claude's installed hook"
    );
    assert!(
        claude_after.contains(&pin_line(&bin_a)),
        "Claude's hook must still pin its own executable after Grok is removed"
    );
    assert!(home.join(".famp/hook-runner.sh").exists());
}

/// The two installed shims come from the same source template but are
/// distinct files on disk: reinstalling one repins only that one.
#[test]
fn reinstalling_one_integration_repins_only_its_own_hook() {
    let tmp = tempfile::TempDir::new().unwrap();
    let home = tmp.path();
    let (_bin_a, bin_b) = setup(home);

    let grok_before = std::fs::read_to_string(home.join(".grok/hooks/famp-await.sh")).unwrap();
    let bin_c = stage_executable(&home.join("bin-c/famp"));
    install_claude(home, &bin_c);

    let claude_after = std::fs::read_to_string(home.join(".claude/hooks/famp-await.sh")).unwrap();
    let runner_after = std::fs::read_to_string(home.join(".famp/hook-runner.sh")).unwrap();
    assert!(claude_after.contains(&pin_line(&bin_c)));
    assert!(runner_after.contains(&pin_line(&bin_c)));
    assert_eq!(
        std::fs::read_to_string(home.join(".grok/hooks/famp-await.sh")).unwrap(),
        grok_before,
        "reinstalling Claude must not repin Grok's hook"
    );
    assert!(
        std::fs::read_to_string(home.join(".grok/hooks/famp-await.sh"))
            .unwrap()
            .contains(&pin_line(&bin_b))
    );
}
