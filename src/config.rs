use std::error::Error;
use std::path::PathBuf;

pub fn application_data_dir() -> Result<PathBuf, Box<dyn Error>> {
    let local_app_data = std::env::var_os("LOCALAPPDATA")
        .ok_or("LOCALAPPDATA is not available on this Windows account")?;
    Ok(PathBuf::from(local_app_data).join("SoundShare"))
}

pub fn identity_path() -> Result<PathBuf, Box<dyn Error>> {
    Ok(application_data_dir()?.join("identity.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_is_stored_outside_the_repository() {
        let identity = identity_path().expect("LOCALAPPDATA should exist during Windows tests");
        assert_eq!(
            identity.file_name().and_then(|name| name.to_str()),
            Some("identity.json")
        );
        assert!(identity
            .components()
            .any(|part| part.as_os_str() == "SoundShare"));
    }
}
