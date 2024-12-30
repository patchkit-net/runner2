use std::fmt;

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Network(reqwest::Error),
    Json(serde_json::Error),
    Zip(zip::result::ZipError),
    DatFile(String),
    FileSystem(String),
    Manifest(String),
    Lockfile(String),
    Permission(String),
    Other(String),
    Which(which::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "I/O error: {}", e),
            Error::Network(e) => write!(f, "Network error: {}", e),
            Error::Json(e) => write!(f, "JSON error: {}", e),
            Error::Zip(e) => write!(f, "ZIP error: {}", e),
            Error::DatFile(s) => write!(f, "DAT file error: {}", s),
            Error::FileSystem(s) => write!(f, "File system error: {}", s),
            Error::Manifest(s) => write!(f, "Manifest error: {}", s),
            Error::Lockfile(s) => write!(f, "Lockfile error: {}", s),
            Error::Permission(s) => write!(f, "Permission error: {}", s),
            Error::Other(s) => write!(f, "{}", s),
            Error::Which(e) => write!(f, "Which error: {}", e),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Error::Network(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Json(err)
    }
}

impl From<zip::result::ZipError> for Error {
    fn from(err: zip::result::ZipError) -> Self {
        Error::Zip(err)
    }
}

impl From<which::Error> for Error {
    fn from(err: which::Error) -> Self {
        Error::Which(err)
    }
} 