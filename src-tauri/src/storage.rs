use std::{fs, path::Path};

pub fn secure_directory(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path).map_err(|e| e.to_string())?;
    permissions(path, 0o700)
}
pub fn secure_file(path: &Path) -> Result<(), String> {
    permissions(path, 0o600)
}
fn permissions(path: &Path, mode: u32) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))
            .map_err(|e| format!("Could not protect {}: {e}", path.display()))?;
    }
    // Windows files inherit the user's profile-directory ACL, including tempfile.
    #[cfg(not(unix))]
    let _ = (path, mode);
    Ok(())
}
