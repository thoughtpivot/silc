//! Install the compiler-pinned `llama-cpp-python` into a Silc-owned venv.
//!
//! The python-build-standalone CPython that Silc provisions bakes
//! `CC=clang` / `CXX=clang++` into its `sysconfig`, so a source build forces
//! clang even on hosts that only have gcc/g++. This module therefore
//!
//! 1. prefers a prebuilt wheel, so most hosts never compile llama.cpp;
//! 2. otherwise builds from source with an explicit `CC`/`CXX`: the user's
//!    own values when set, else clang when present, else gcc/g++, else cc/c++;
//! 3. fails early, naming the missing tool, when no compiler can be found.

use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Index that hosts prebuilt CPU wheels of `llama-cpp-python`. PyPI carries
/// only the source distribution.
const PREBUILT_WHEEL_INDEX: &str = "https://abetlen.github.io/llama-cpp-python/whl/cpu";

/// Set to `1` to skip the prebuilt wheel and always compile from source.
const SOURCE_ONLY_ENV: &str = "SILC_LLAMA_CPP_SOURCE";

/// `(C compiler, C++ compiler)` pairs in preference order.
const COMPILER_FAMILIES: [(&str, &str); 3] = [("clang", "clang++"), ("gcc", "g++"), ("cc", "c++")];

const COMPILER_INSTALL_HINT: &str = "Install a C/C++ toolchain, then retry: \
Debian/Ubuntu `sudo apt-get install build-essential`, \
Fedora/RHEL `sudo dnf install gcc gcc-c++`, \
Alpine `apk add build-base`, \
macOS `xcode-select --install`. \
Or set CC and CXX to your compilers.";

/// Install the requirements file at `requirements` with `python`.
///
/// `pip_stdout` supplies the stdout handle for pip, because callers differ on
/// whether stdout carries a command result.
pub fn install_llama_cpp_python(
    python: &Path,
    requirements: &Path,
    pip_stdout: fn() -> Stdio,
) -> Result<(), String> {
    let source_only = env::var(SOURCE_ONLY_ENV).is_ok_and(|v| v.trim() == "1");
    if !source_only && install_prebuilt_wheel(python, requirements, pip_stdout) {
        // Dependencies come from PyPI; the pinned wheel already satisfies
        // llama-cpp-python, so nothing is compiled.
        return pip_install(python, requirements, &[], &[], pip_stdout)
            .map_err(|e| format!("install llama-cpp-python dependencies: {e}"));
    }

    if !source_only {
        eprintln!("no prebuilt llama-cpp-python wheel for this platform; building from source");
    }
    let overrides = select_compiler_env(env::var("CC").ok(), env::var("CXX").ok(), &|name| {
        find_on_path(name).is_some()
    })?;
    pip_install(python, requirements, &[], &overrides, pip_stdout)
        .map_err(|_| "Silc install of compiler-pinned llama-cpp-python failed".to_string())
}

/// Try to install only a prebuilt wheel of the pinned version. Returns whether
/// it succeeded; any failure falls back to a source build.
fn install_prebuilt_wheel(python: &Path, requirements: &Path, pip_stdout: fn() -> Stdio) -> bool {
    pip_install(
        python,
        requirements,
        &[
            "--only-binary=:all:",
            "--no-deps",
            "--index-url",
            PREBUILT_WHEEL_INDEX,
        ],
        &[],
        pip_stdout,
    )
    .is_ok()
}

fn pip_install(
    python: &Path,
    requirements: &Path,
    extra_args: &[&str],
    envs: &[(&'static str, String)],
    pip_stdout: fn() -> Stdio,
) -> Result<(), String> {
    let mut cmd = Command::new(python);
    cmd.args(["-m", "pip", "install", "--disable-pip-version-check"])
        .args(extra_args)
        .arg("-r")
        .arg(requirements)
        .stdout(pip_stdout());
    for (key, value) in envs {
        cmd.env(key, value);
    }
    let status = cmd.status().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("pip exited with {status}"))
    }
}

/// Decide the `CC`/`CXX` to pass to a source build.
///
/// A non-empty user value is respected as is, but its program must exist. Any
/// variable the user left unset gets a detected compiler. The returned pairs
/// are the variables to set on the pip process.
fn select_compiler_env(
    user_cc: Option<String>,
    user_cxx: Option<String>,
    has: &dyn Fn(&str) -> bool,
) -> Result<Vec<(&'static str, String)>, String> {
    let user_cc = user_cc.filter(|v| !v.trim().is_empty());
    let user_cxx = user_cxx.filter(|v| !v.trim().is_empty());

    for (var, value) in [("CC", &user_cc), ("CXX", &user_cxx)] {
        if let Some(value) = value {
            let program = value.split_whitespace().next().unwrap_or(value);
            if !has(program) {
                return Err(format!(
                    "{var}={value} is set but `{program}` was not found. \
Unset {var} to let Silc pick a compiler, or point it at an installed one. {COMPILER_INSTALL_HINT}"
                ));
            }
        }
    }

    if user_cc.is_some() && user_cxx.is_some() {
        return Ok(Vec::new());
    }

    let mut overrides = Vec::new();
    match detect_family(has) {
        Ok((cc, cxx)) => {
            if user_cc.is_none() {
                overrides.push(("CC", cc.to_string()));
            }
            if user_cxx.is_none() {
                overrides.push(("CXX", cxx.to_string()));
            }
        }
        Err(missing) => {
            let needed_cc = user_cc.is_none();
            let needed_cxx = user_cxx.is_none();
            let still_missing: Vec<&str> = missing
                .iter()
                .copied()
                .filter(|tool| {
                    let is_cxx = COMPILER_FAMILIES.iter().any(|(_, cxx)| cxx == tool);
                    if is_cxx {
                        needed_cxx
                    } else {
                        needed_cc
                    }
                })
                .collect();
            return Err(format!(
                "cannot build llama-cpp-python from source: missing {}. {COMPILER_INSTALL_HINT}",
                still_missing.join(" and ")
            ));
        }
    }
    Ok(overrides)
}

/// The first complete compiler pair on the host, or the tools to install.
fn detect_family(
    has: &dyn Fn(&str) -> bool,
) -> Result<(&'static str, &'static str), Vec<&'static str>> {
    for (cc, cxx) in COMPILER_FAMILIES {
        if has(cc) && has(cxx) {
            return Ok((cc, cxx));
        }
    }
    // Report the gcc-family tools that are absent, so the message names what
    // to install. If a partial family exists, name only its missing half.
    for (cc, cxx) in COMPILER_FAMILIES {
        match (has(cc), has(cxx)) {
            (true, false) => return Err(vec![cxx]),
            (false, true) => return Err(vec![cc]),
            _ => {}
        }
    }
    Err(vec!["gcc", "g++"])
}

fn find_on_path(program: &str) -> Option<PathBuf> {
    let candidate = Path::new(program);
    if candidate.components().count() > 1 {
        return is_executable(candidate).then(|| candidate.to_path_buf());
    }
    let path: OsString = env::var_os("PATH")?;
    env::split_paths(&path)
        .map(|dir| dir.join(program))
        .find(|p| is_executable(p))
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host(tools: &'static [&'static str]) -> impl Fn(&str) -> bool {
        move |name| tools.contains(&name)
    }

    fn pairs(v: Vec<(&'static str, String)>) -> Vec<(&'static str, &'static str)> {
        v.into_iter()
            .map(|(k, val)| (k, Box::leak(val.into_boxed_str()) as &'static str))
            .collect()
    }

    #[test]
    fn prefers_clang_when_present() {
        let got =
            select_compiler_env(None, None, &host(&["clang", "clang++", "gcc", "g++"])).unwrap();
        assert_eq!(pairs(got), vec![("CC", "clang"), ("CXX", "clang++")]);
    }

    #[test]
    fn gcc_only_host_overrides_the_pbs_clang_default() {
        let got = select_compiler_env(None, None, &host(&["gcc", "g++", "cc", "c++"])).unwrap();
        assert_eq!(pairs(got), vec![("CC", "gcc"), ("CXX", "g++")]);
    }

    #[test]
    fn falls_back_to_cc_and_c_plus_plus() {
        let got = select_compiler_env(None, None, &host(&["cc", "c++"])).unwrap();
        assert_eq!(pairs(got), vec![("CC", "cc"), ("CXX", "c++")]);
    }

    #[test]
    fn user_set_cc_and_cxx_are_respected() {
        let got = select_compiler_env(
            Some("gcc-13".into()),
            Some("g++-13".into()),
            &host(&["gcc-13", "g++-13"]),
        )
        .unwrap();
        assert!(got.is_empty());
    }

    #[test]
    fn user_set_cc_only_still_gets_a_detected_cxx() {
        let got = select_compiler_env(Some("gcc".into()), None, &host(&["gcc", "g++"])).unwrap();
        assert_eq!(pairs(got), vec![("CXX", "g++")]);
    }

    #[test]
    fn user_compiler_wrapper_checks_the_first_word() {
        let got = select_compiler_env(
            Some("ccache gcc".into()),
            Some("ccache g++".into()),
            &host(&["ccache"]),
        )
        .unwrap();
        assert!(got.is_empty());
    }

    #[test]
    fn empty_user_values_count_as_unset() {
        let got = select_compiler_env(
            Some("  ".into()),
            Some(String::new()),
            &host(&["gcc", "g++"]),
        )
        .unwrap();
        assert_eq!(pairs(got), vec![("CC", "gcc"), ("CXX", "g++")]);
    }

    #[test]
    fn missing_user_compiler_is_named() {
        let err =
            select_compiler_env(Some("nope-cc".into()), None, &host(&["gcc", "g++"])).unwrap_err();
        assert!(err.contains("CC=nope-cc"), "{err}");
        assert!(err.contains("`nope-cc` was not found"), "{err}");
    }

    #[test]
    fn no_compiler_names_gcc_and_gxx_with_install_hint() {
        let err = select_compiler_env(None, None, &host(&[])).unwrap_err();
        assert!(err.contains("missing gcc and g++"), "{err}");
        assert!(err.contains("build-essential"), "{err}");
        assert!(err.contains("xcode-select --install"), "{err}");
    }

    #[test]
    fn partial_toolchain_names_only_the_missing_half() {
        let err = select_compiler_env(None, None, &host(&["gcc"])).unwrap_err();
        assert!(err.contains("missing g++."), "{err}");
    }
}
