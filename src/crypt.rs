//! Config files kept with their secrets go into one age file, `files.age`, encrypted with a
//! password: one scrypt run per export or restore, and `age -d files.age` opens it without repoke.

use std::collections::BTreeMap;
use std::env;

use age::secrecy::SecretString;
use inquire::{InquireError, Password, PasswordDisplayMode};

pub const FILE: &str = "files.age";
pub const ENV: &str = "REPOKE_PASSWORD";

/// File contents by path relative to the home directory.
pub type Bundle = BTreeMap<String, String>;

pub fn from_env() -> Option<SecretString> {
    env::var(ENV)
        .ok()
        .filter(|p| !p.is_empty())
        .map(SecretString::from)
}

pub fn ask(message: &str, confirm: bool) -> Result<SecretString, InquireError> {
    let prompt = Password::new(message).with_display_mode(PasswordDisplayMode::Masked);
    let prompt = if confirm {
        prompt.with_custom_confirmation_message("Repeat it:")
    } else {
        prompt.without_confirmation()
    };
    prompt.prompt().map(SecretString::from)
}

pub fn seal(bundle: &Bundle, password: &SecretString) -> Result<Vec<u8>, String> {
    let text = toml::to_string(bundle).map_err(|e| e.to_string())?;
    let recipient = age::scrypt::Recipient::new(password.clone());
    age::encrypt(&recipient, text.as_bytes()).map_err(|e| e.to_string())
}

pub fn open(data: &[u8], password: &SecretString) -> Result<Bundle, String> {
    let identity = age::scrypt::Identity::new(password.clone());
    let text = age::decrypt(&identity, data).map_err(|e| match e {
        age::DecryptError::DecryptionFailed | age::DecryptError::NoMatchingKeys => {
            "wrong password".to_string()
        }
        e => e.to_string(),
    })?;
    let text = String::from_utf8(text).map_err(|e| e.to_string())?;
    toml::from_str(&text).map_err(|e| format!("{FILE}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_what_it_sealed_and_only_with_the_password() {
        let bundle = Bundle::from([(".npmrc".to_string(), "//r/:_authToken=abc\n".to_string())]);
        let sealed = seal(&bundle, &SecretString::from("right".to_string())).unwrap();
        assert!(!String::from_utf8_lossy(&sealed).contains("abc"));
        let opened = open(&sealed, &SecretString::from("right".to_string())).unwrap();
        assert_eq!(opened, bundle);
        let wrong = open(&sealed, &SecretString::from("wrong".to_string()));
        assert_eq!(wrong.unwrap_err(), "wrong password");
    }
}
