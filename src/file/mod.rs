use crate::Result;
use directories::BaseDirs;
use std::fs::{self, File};
use std::io::{self, Write, Read};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use zip::ZipArchive;
#[cfg(target_os = "macos")]
use std::os::unix::fs::PermissionsExt;
use log::{debug, info, warn};

pub struct FileManager {
    install_dir: PathBuf,
    installed_files: Vec<PathBuf>,
}

impl FileManager {
    pub fn new(secret_slug: &str) -> Result<Self> {
        let install_dir = if cfg!(target_os = "macos") {
            let base_dirs = BaseDirs::new()
                .ok_or_else(|| crate::Error::FileSystem("Could not determine base directories".into()))?;
            
            base_dirs
                .data_dir()
                .join("PatchKit")
                .join("Apps")
                .join(secret_slug)
        } else {
            PathBuf::from("app")
        };

        Ok(Self {
            install_dir,
            installed_files: Vec::new(),
        })
    }

    pub fn get_install_dir(&self) -> &Path {
        &self.install_dir
    }

    pub fn create_install_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.install_dir)?;
        Ok(())
    }

    pub fn get_current_version(&self) -> Result<Option<String>> {
        let version_file = self.install_dir.join("version.txt");
        if !version_file.exists() {
            return Ok(None);
        }

        let mut content = String::new();
        File::open(version_file)?.read_to_string(&mut content)?;
        Ok(Some(content.trim().to_string()))
    }

    pub fn save_version(&self, version: &str) -> Result<()> {
        let version_file = self.install_dir.join("version.txt");
        let mut file = File::create(version_file)?;
        write!(file, "{}", version)?;
        Ok(())
    }

    pub fn needs_update(&self, new_version: &str) -> Result<bool> {
        match self.get_current_version()? {
            Some(current_version) => Ok(current_version != new_version),
            None => Ok(true)
        }
    }

    pub fn extract_zip<P: AsRef<Path>>(&mut self, zip_path: P, destination: P) -> Result<()> {
        let file = File::open(zip_path)?;
        let mut archive = ZipArchive::new(file)?;

        // Clear the installed files list before new extraction
        self.installed_files.clear();

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let outpath = destination.as_ref().join(file.mangled_name());

            if file.name().ends_with('/') {
                fs::create_dir_all(&outpath)?;
            } else {
                if let Some(p) = outpath.parent() {
                    fs::create_dir_all(p)?;
                }
                let mut outfile = File::create(&outpath)?;
                io::copy(&mut file, &mut outfile)?;

                #[cfg(target_os = "macos")]
                {
                    // Check if the file is in Contents/MacOS directory
                    if outpath.to_string_lossy().contains("Contents/MacOS") {
                        // Set executable permissions (read/write/execute for owner, read/execute for group and others)
                        let perms = fs::Permissions::from_mode(0o755);
                        fs::set_permissions(&outpath, perms)?;
                    }
                }
            }

            debug!("Extracted: {}", outpath.display());
            self.installed_files.push(outpath);
        }

        Ok(())
    }

    pub fn remove_old_files(&self) -> Result<()> {
        info!("Removing old files");
        for path in self.installed_files.iter().rev() {
            if path.is_file() {
                if let Err(e) = fs::remove_file(path) {
                    warn!("Failed to remove file {}: {}", path.display(), e);
                } else {
                    debug!("Removed file: {}", path.display());
                }
            } else if path.is_dir() {
                // Only remove directory if it's empty
                if fs::read_dir(path)?.next().is_none() {
                    if let Err(e) = fs::remove_dir(path) {
                        warn!("Failed to remove directory {}: {}", path.display(), e);
                    } else {
                        debug!("Removed directory: {}", path.display());
                    }
                } else {
                    debug!("Skipping non-empty directory: {}", path.display());
                }
            }
        }
        Ok(())
    }

    pub fn create_lockfile<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let mut file = File::create(path)?;
        write!(file, "{}", std::process::id())?;
        Ok(())
    }

    pub fn check_lockfile<P: AsRef<Path>>(&self, path: P) -> Result<bool> {
        let path = path.as_ref();
        
        if !path.exists() {
            return Ok(false);
        }

        if let Ok(metadata) = fs::metadata(path) {
            if let Ok(modified) = metadata.modified() {
                if let Ok(duration) = SystemTime::now().duration_since(modified) {
                    if duration > Duration::from_secs(60) {
                        fs::remove_file(path)?;
                        return Ok(false);
                    }
                }
            }
        }

        Ok(true)
    }

    pub fn delete_lockfile<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        fs::remove_file(path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_create_install_dir() {
        let manager = FileManager::new("test123").unwrap();
        assert!(manager.create_install_dir().is_ok());
        assert!(manager.get_install_dir().exists());
        fs::remove_dir_all(manager.get_install_dir()).unwrap_or(());
    }

    #[test]
    fn test_lockfile_operations() {
        let manager = FileManager::new("test123").unwrap();
        let temp_dir = tempdir().unwrap();
        let lockfile_path = temp_dir.path().join("test.lock");

        // Create lockfile
        assert!(manager.create_lockfile(&lockfile_path).is_ok());
        assert!(lockfile_path.exists());

        // Check lockfile
        assert!(manager.check_lockfile(&lockfile_path).unwrap());

        // Delete lockfile
        assert!(manager.delete_lockfile(&lockfile_path).is_ok());
        assert!(!lockfile_path.exists());
    }

    #[test]
    fn test_extract_zip() {
        let mut manager = FileManager::new("test123").unwrap();
        let temp_dir = tempdir().unwrap();
        let zip_path = temp_dir.path().join("test.zip");
        let extract_dir = temp_dir.path().join("extracted");

        // Create a test zip file
        let mut zip = zip::ZipWriter::new(File::create(&zip_path).unwrap());
        zip.start_file("test.txt", zip::write::FileOptions::default()).unwrap();
        zip.write_all(b"test content").unwrap();
        zip.finish().unwrap();

        // Extract the zip file
        assert!(manager.extract_zip(&zip_path, &extract_dir).is_ok());
        assert!(extract_dir.join("test.txt").exists());
    }

    #[test]
    fn test_version_management() {
        let manager = FileManager::new("test123").unwrap();
        manager.create_install_dir().unwrap();

        // Initially there should be no version
        assert!(manager.get_current_version().unwrap().is_none());

        // Save version and verify it
        manager.save_version("1.0.0").unwrap();
        assert_eq!(manager.get_current_version().unwrap().unwrap(), "1.0.0");

        // Check if update is needed
        assert!(manager.needs_update("2.0.0").unwrap());
        assert!(!manager.needs_update("1.0.0").unwrap());

        // Clean up
        fs::remove_dir_all(manager.get_install_dir()).unwrap_or(());
    }

    #[test]
    fn test_file_cleanup() {
        let mut manager = FileManager::new("test123").unwrap();
        let temp_dir = tempdir().unwrap();
        let zip_path = temp_dir.path().join("test.zip");
        let extract_dir = temp_dir.path().join("extract");

        // Create a test zip file with multiple files and directories
        let mut zip = zip::ZipWriter::new(File::create(&zip_path).unwrap());
        
        // Add a file in a directory
        zip.add_directory("test_dir", Default::default()).unwrap();
        zip.start_file("test_dir/test1.txt", Default::default()).unwrap();
        zip.write_all(b"test content 1").unwrap();
        
        // Add another file
        zip.start_file("test2.txt", Default::default()).unwrap();
        zip.write_all(b"test content 2").unwrap();
        
        zip.finish().unwrap();

        // Extract the zip file
        manager.extract_zip(&zip_path, &extract_dir).unwrap();

        // Verify files were extracted
        assert!(extract_dir.join("test_dir").join("test1.txt").exists());
        assert!(extract_dir.join("test2.txt").exists());

        // Remove old files
        manager.remove_old_files().unwrap();

        // Verify files were removed
        assert!(!extract_dir.join("test_dir").join("test1.txt").exists());
        assert!(!extract_dir.join("test2.txt").exists());
        // Directory should be removed as it's empty
        assert!(!extract_dir.join("test_dir").exists());
    }
} 