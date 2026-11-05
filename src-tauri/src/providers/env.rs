//! Environment variable lookup that also sees variables set after launch.
//!
//! A running process keeps the environment it started with, so on Windows a
//! variable created later with `setx NAME value` (stored in
//! `HKCU\Environment`) is also read from the registry. That lets a user add
//! a token/key and have QuotaPulse pick it up on the next refresh without a
//! restart.

/// Value of `name` (trimmed, non-empty) and where it came from.
pub(crate) fn lookup(name: &str) -> Option<(String, &'static str)> {
    if let Ok(value) = std::env::var(name) {
        let value = value.trim();
        if !value.is_empty() {
            return Some((value.to_string(), "process environment"));
        }
    }
    user_env_var(name)
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .map(|v| (v, "user environment"))
}

#[cfg(target_os = "windows")]
fn user_env_var(name: &str) -> Option<String> {
    use winreg::enums::HKEY_CURRENT_USER;
    winreg::RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey("Environment")
        .ok()?
        .get_value::<String, _>(name)
        .ok()
}

#[cfg(not(target_os = "windows"))]
fn user_env_var(_name: &str) -> Option<String> {
    None
}
