use std::{collections::BTreeMap, env, fs, path::PathBuf};

use zeroize::Zeroizing;

use crate::error::{Error, Result};

/// Signing material returned at the trust boundary.
///
/// Secret values are zeroed on drop. A future enclave provider can implement
/// [`CredentialProvider`] without changing service or transport code.
pub struct Credentials {
    access_key_id: String,
    secret_access_key: Zeroizing<String>,
    session_token: Option<Zeroizing<String>>,
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Credentials")
            .field("access_key_id", &"[redacted]")
            .field("secret_access_key", &"[redacted]")
            .field(
                "session_token",
                &self.session_token.as_ref().map(|_| "[redacted]"),
            )
            .finish()
    }
}

impl Credentials {
    pub fn new(
        access_key_id: String,
        secret_access_key: String,
        session_token: Option<String>,
    ) -> Result<Self> {
        if access_key_id.is_empty() || secret_access_key.is_empty() {
            return Err(Error::Credentials(
                "access key and secret key are both required".into(),
            ));
        }
        Ok(Self {
            access_key_id,
            secret_access_key: Zeroizing::new(secret_access_key),
            session_token: session_token
                .filter(|value| !value.is_empty())
                .map(Zeroizing::new),
        })
    }

    pub fn access_key_id(&self) -> &str {
        &self.access_key_id
    }

    pub(crate) fn secret_access_key(&self) -> &str {
        self.secret_access_key.as_str()
    }

    pub fn session_token(&self) -> Option<&str> {
        self.session_token.as_deref().map(String::as_str)
    }
}

pub trait CredentialProvider: Send + Sync {
    fn provide(&self) -> Result<Credentials>;
}

/// Loads environment credentials first, then a named shared-credentials profile.
/// It deliberately does not invoke credential-process, SSO, ECS, or IMDS helpers.
#[derive(Debug, Clone)]
pub struct EnvironmentProvider {
    profile: String,
    credentials_file: Option<PathBuf>,
    allow_environment: bool,
}

impl Default for EnvironmentProvider {
    fn default() -> Self {
        Self {
            profile: env::var("AWS_PROFILE").unwrap_or_else(|_| "default".into()),
            credentials_file: env::var_os("AWS_SHARED_CREDENTIALS_FILE").map(PathBuf::from),
            allow_environment: true,
        }
    }
}

impl EnvironmentProvider {
    pub fn with_profile(profile: impl Into<String>) -> Self {
        Self {
            profile: profile.into(),
            credentials_file: None,
            allow_environment: true,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_file(profile: impl Into<String>, path: PathBuf) -> Self {
        Self {
            profile: profile.into(),
            credentials_file: Some(path),
            allow_environment: false,
        }
    }

    fn load_environment(&self) -> Result<Option<Credentials>> {
        let access = env::var("AWS_ACCESS_KEY_ID").ok();
        let secret = env::var("AWS_SECRET_ACCESS_KEY").ok();
        match (access, secret) {
            (Some(access), Some(secret)) => {
                Credentials::new(access, secret, env::var("AWS_SESSION_TOKEN").ok()).map(Some)
            }
            (None, None) => Ok(None),
            _ => Err(Error::Credentials(
                "AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY must be set together".into(),
            )),
        }
    }

    fn credentials_path(&self) -> Result<PathBuf> {
        if let Some(path) = &self.credentials_file {
            return Ok(path.clone());
        }
        let home = env::var_os("HOME").ok_or_else(|| {
            Error::Credentials("HOME is unset and no credentials file was specified".into())
        })?;
        Ok(PathBuf::from(home).join(".aws/credentials"))
    }
}

impl CredentialProvider for EnvironmentProvider {
    fn provide(&self) -> Result<Credentials> {
        if self.allow_environment
            && let Some(credentials) = self.load_environment()?
        {
            return Ok(credentials);
        }
        let path = self.credentials_path()?;
        let contents = fs::read_to_string(&path).map_err(|error| {
            Error::Credentials(format!("cannot read {}: {error}", path.display()))
        })?;
        let profiles = parse_ini(&contents);
        let profile = profiles.get(&self.profile).ok_or_else(|| {
            Error::Credentials(format!(
                "profile {:?} is absent from {}",
                self.profile,
                path.display()
            ))
        })?;
        Credentials::new(
            profile
                .get("aws_access_key_id")
                .cloned()
                .unwrap_or_default(),
            profile
                .get("aws_secret_access_key")
                .cloned()
                .unwrap_or_default(),
            profile.get("aws_session_token").cloned(),
        )
    }
}

fn parse_ini(contents: &str) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut result = BTreeMap::new();
    let mut section: Option<String> = None;
    for source_line in contents.lines() {
        let line = source_line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|value| value.strip_suffix(']'))
        {
            section = Some(name.trim().to_owned());
            result
                .entry(name.trim().to_owned())
                .or_insert_with(BTreeMap::new);
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if let Some(name) = &section {
            result
                .entry(name.clone())
                .or_default()
                .insert(key.trim().to_owned(), value.trim().to_owned());
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_selected_shared_credentials_profile() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("credentials");
        fs::write(&path, "[default]\naws_access_key_id = A\naws_secret_access_key = B\n\n[tee]\naws_access_key_id=TEEKEY\naws_secret_access_key=TEESECRET\naws_session_token=TOKEN\n").unwrap();
        let credentials = EnvironmentProvider::with_file("tee", path)
            .provide()
            .unwrap();
        assert_eq!(credentials.access_key_id(), "TEEKEY");
        assert_eq!(credentials.secret_access_key(), "TEESECRET");
        assert_eq!(credentials.session_token(), Some("TOKEN"));
        let debug = format!("{credentials:?}");
        assert!(!debug.contains("TEEKEY"));
        assert!(!debug.contains("TEESECRET"));
        assert!(!debug.contains("TOKEN"));
    }
}
