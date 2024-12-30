use crate::Result;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct Manifest {
    pub manifest_version: i32,
    pub target: String,
    pub target_arguments: Vec<TargetArgument>,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct TargetArgument {
    pub value: Vec<String>,
}

#[derive(Debug)]
pub struct ManifestManager {
    manifest: Manifest,
    variables: HashMap<String, String>,
}

impl ManifestManager {
    pub fn new(manifest_content: &str) -> Result<Self> {
        let manifest: Manifest = serde_json::from_str(manifest_content)?;
        Ok(Self {
            manifest,
            variables: HashMap::new(),
        })
    }

    pub fn set_variable(&mut self, key: &str, value: String) {
        self.variables.insert(key.to_string(), value);
    }

    pub fn get_target(&self) -> Result<PathBuf> {
        let target = self.resolve_variables(&self.manifest.target)?;
        Ok(PathBuf::from(target))
    }

    pub fn get_arguments(&self) -> Result<Vec<String>> {
        let mut resolved_args = Vec::new();
        
        for arg in &self.manifest.target_arguments {
            for value in &arg.value {
                let resolved = self.resolve_variables(value)?;
                resolved_args.push(resolved);
            }
        }
        
        Ok(resolved_args)
    }

    fn resolve_variables(&self, input: &str) -> Result<String> {
        let mut result = input.to_string();
        
        for (key, value) in &self.variables {
            let placeholder = format!("{{{}}}", key);
            result = result.replace(&placeholder, value);
        }
        
        // Check if there are any unresolved variables
        if result.contains('{') && result.contains('}') {
            return Err(crate::Error::Manifest(
                "Unresolved variables in manifest".into()
            ));
        }
        
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_MANIFEST: &str = r#"{
        "manifest_version": 4,
        "target": "{exedir}/Patcher.exe",
        "target_arguments": [
            {
                "value": ["--installdir", "{installdir}"]
            },
            {
                "value": ["--lockfile", "{lockfile}"]
            }
        ],
        "capabilities": ["pack1_compression_lzma2"]
    }"#;

    #[test]
    fn test_manifest_parsing() {
        let manager = ManifestManager::new(SAMPLE_MANIFEST).unwrap();
        assert_eq!(manager.manifest.manifest_version, 4);
    }

    #[test]
    fn test_variable_resolution() {
        let mut manager = ManifestManager::new(SAMPLE_MANIFEST).unwrap();
        
        manager.set_variable("exedir", "/path/to/exe".into());
        manager.set_variable("installdir", "/path/to/install".into());
        manager.set_variable("lockfile", "/path/to/lock".into());
        
        let target = manager.get_target().unwrap();
        assert_eq!(target, PathBuf::from("/path/to/exe/Patcher.exe"));
        
        let args = manager.get_arguments().unwrap();
        assert_eq!(args[0], "--installdir");
        assert_eq!(args[1], "/path/to/install");
        assert_eq!(args[2], "--lockfile");
        assert_eq!(args[3], "/path/to/lock");
    }

    #[test]
    fn test_unresolved_variables() {
        let manager = ManifestManager::new(SAMPLE_MANIFEST).unwrap();
        assert!(manager.get_target().is_err());
    }
} 