#[cfg(feature = "bz")]
mod bzip2;
#[cfg(feature = "gz")]
mod gzip;
#[cfg(feature = "lz")]
mod lz4;
#[cfg(feature = "s3")]
pub mod s3;

use crate::OneIoError;
#[cfg(feature = "remote")]
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Lines, Read, Write};
use std::path::Path;

pub trait OneIOCompression {
    fn get_reader(raw_reader: Box<dyn Read + Send>) -> Result<Box<dyn Read + Send>, OneIoError>;
    fn get_writer(raw_writer: BufWriter<File>) -> Result<Box<dyn Write>, OneIoError>;
}

fn get_reader_raw(path: &str) -> Result<Box<dyn Read>, OneIoError> {
    #[cfg(feature = "remote")]
    let raw_reader: Box<dyn Read> = if path.starts_with("http") {
        let response = get_remote_http_raw(path, HashMap::new())?;
        Box::new(response)
    } else if path.starts_with("ftp") {
        let response = get_remote_ftp_raw(path)?;
        Box::new(response)
    } else {
        Box::new(std::fs::File::open(path)?)
    };
    #[cfg(not(feature = "remote"))]
    let raw_reader: Box<dyn Read> = Box::new(std::fs::File::open(path)?);
    Ok(raw_reader)
}

#[cfg(feature = "remote")]
fn get_remote_http_raw(
    path: &str,
    header: HashMap<String, String>,
) -> Result<reqwest::blocking::Response, OneIoError> {
    let headers: reqwest::header::HeaderMap = (&header).try_into().expect("invalid headers");
    let client = reqwest::blocking::Client::builder()
        .default_headers(headers)
        .build()?;
    let res = client
        .execute(client.get(path).build()?)?
        .error_for_status()?;
    Ok(res)
}

#[cfg(feature = "remote")]
fn get_remote_ftp_raw(path: &str) -> Result<Box<dyn Read + Send>, OneIoError> {
    if !path.starts_with("ftp://") {
        return Err(OneIoError::NotSupported(path.to_string()));
    }

    let parts = path.split('/').collect::<Vec<&str>>();
    let socket = match parts[2].contains(':') {
        true => parts[2].to_string(),
        false => format!("{}:21", parts[2]),
    };
    let path = parts[3..].join("/");

    let mut ftp_stream = suppaftp::FtpStream::connect(socket)?;
    ftp_stream.login("anonymous", "oneio").unwrap();
    let reader = Box::new(ftp_stream.retr_as_stream(path.as_str())?);
    Ok(reader)
}

#[cfg(feature = "remote")]
/// get a reader for remote content with the capability to specify headers.
///
/// Example usage:
/// ```no_run
/// use std::collections::HashMap;
/// let mut reader = oneio::get_remote_reader(
///   "https://SOME_REMOTE_RESOURCE_PROTECTED_BY_ACCESS_TOKEN",
///   HashMap::from([("X-Custom-Auth-Key".to_string(), "TOKEN".to_string())])
/// ).unwrap();
/// let mut text = "".to_string();
/// reader.read_to_string(&mut text).unwrap();
/// println!("{}", text);
/// ```
pub fn get_remote_reader(
    path: &str,
    header: HashMap<String, String>,
) -> Result<Box<dyn Read + Send>, OneIoError> {
    let raw_reader: Box<dyn Read + Send> = Box::new(get_remote_http_raw(path, header)?);
    let file_type = *path.split('.').collect::<Vec<&str>>().last().unwrap();
    match file_type {
        #[cfg(feature = "gz")]
        "gz" | "gzip" => gzip::OneIOGzip::get_reader(raw_reader),
        #[cfg(feature = "bz")]
        "bz2" | "bz" => bzip2::OneIOBzip2::get_reader(raw_reader),
        #[cfg(feature = "lz4")]
        "lz4" | "lz" => lz4::OneIOLz4::get_reader(raw_reader),
        _ => {
            // unknown file type of file {}. try to read as uncompressed file
            Ok(Box::new(raw_reader))
        }
    }
}

#[cfg(feature = "remote")]
pub fn download(
    remote_path: &str,
    local_path: &str,
    header: Option<HashMap<String, String>>,
) -> Result<(), OneIoError> {
    let mut response = get_remote_http_raw(remote_path, header.unwrap_or_default())?;
    let mut writer = get_writer_raw(local_path)?;

    response.copy_to(&mut writer)?;
    Ok(())
}

/// Convenient function to directly read remote or local content to a String
pub fn read_to_string(path: &str) -> Result<String, OneIoError> {
    let mut reader = get_reader(path)?;
    let mut content = String::new();
    reader.read_to_string(&mut content)?;
    Ok(content)
}

#[cfg(feature = "json")]
/// Convenient function to directly read remote or local JSON content to a struct
pub fn read_json_struct<T: serde::de::DeserializeOwned>(path: &str) -> Result<T, OneIoError> {
    let reader = get_reader(path)?;
    let res: T = serde_json::from_reader(reader)?;
    Ok(res)
}

/// convenient function to read a file and returns a line iterator.
pub fn read_lines(path: &str) -> Result<Lines<BufReader<Box<dyn Read + Send>>>, OneIoError> {
    let reader = get_reader(path)?;
    let buf_reader = BufReader::new(reader);
    Ok(buf_reader.lines())
}

/// get a generic Box<dyn Read> reader
pub fn get_reader(path: &str) -> Result<Box<dyn Read + Send>, OneIoError> {
    #[cfg(feature = "remote")]
    let raw_reader: Box<dyn Read + Send> = if path.starts_with("http") {
        let response = get_remote_http_raw(path, HashMap::new())?;
        Box::new(response)
    } else if path.starts_with("ftp") {
        let response = get_remote_ftp_raw(path)?;
        Box::new(response)
    } else {
        Box::new(std::fs::File::open(path)?)
    };
    #[cfg(not(feature = "remote"))]
    let raw_reader: Box<dyn Read + Send> = Box::new(std::fs::File::open(path)?);

    let file_type = *path.split('.').collect::<Vec<&str>>().last().unwrap();
    match file_type {
        #[cfg(feature = "gz")]
        "gz" | "gzip" => gzip::OneIOGzip::get_reader(raw_reader),
        #[cfg(feature = "bz")]
        "bz2" | "bz" => bzip2::OneIOBzip2::get_reader(raw_reader),
        #[cfg(feature = "lz4")]
        "lz4" | "lz" => lz4::OneIOLz4::get_reader(raw_reader),
        _ => {
            // unknown file type of file {}. try to read as uncompressed file
            Ok(Box::new(raw_reader))
        }
    }
}

/// get file reader with local cache.
///
/// parameters:
/// * `path`: file path to open, remote or local
/// * `cache_dir`: path str to cache directory
/// * `cache_file_name`: optional file name for cache file, default to use the same filename as the to-read file
/// * `force_cache`: whether to force refresh cache file if a local cache file already exists
pub fn get_cache_reader(
    path: &str,
    cache_dir: &str,
    cache_file_name: Option<String>,
    force_cache: bool,
) -> Result<Box<dyn Read + Send>, OneIoError> {
    let dir_path = std::path::Path::new(cache_dir);
    if !dir_path.is_dir() {
        match std::fs::create_dir_all(dir_path) {
            Ok(_) => {}
            Err(e) => {
                return Err(OneIoError::CacheIoError(format!(
                    "cache directory creation failed: {}",
                    e
                )))
            }
        }
    }

    let cache_file_name = match cache_file_name {
        None => path
            .split('/')
            .collect::<Vec<&str>>()
            .into_iter()
            .last()
            .unwrap()
            .to_string(),
        Some(p) => p,
    };

    let cache_file_path = format!("{}/{}", cache_dir, cache_file_name);

    // if cache file already exists
    if !force_cache && std::path::Path::new(cache_file_path.as_str()).exists() {
        return get_reader(cache_file_path.as_str());
    }

    // read all to cache file, no encode/decode happens
    let mut reader = get_reader_raw(path)?;
    let mut data: Vec<u8> = vec![];
    reader.read_to_end(&mut data)?;
    let mut writer = get_writer_raw(cache_file_path.as_str())?;
    writer.write_all(&data).unwrap();
    drop(writer);

    // return reader from cache file
    get_reader(cache_file_path.as_str())
}

fn get_writer_raw(path: &str) -> Result<BufWriter<File>, OneIoError> {
    let path = Path::new(path);
    if let Some(prefix) = path.parent() {
        std::fs::create_dir_all(prefix)?;
    }
    let output_file = BufWriter::new(File::create(path)?);
    Ok(output_file)
}

pub fn get_writer(path: &str) -> Result<Box<dyn Write>, OneIoError> {
    let output_file = BufWriter::new(File::create(path)?);

    let file_type = *path.split('.').collect::<Vec<&str>>().last().unwrap();
    match file_type {
        #[cfg(feature = "gz")]
        "gz" | "gzip" => gzip::OneIOGzip::get_writer(output_file),
        #[cfg(feature = "bz")]
        "bz2" | "bz" => bzip2::OneIOBzip2::get_writer(output_file),
        #[cfg(feature = "lz4")]
        "lz4" | "lz" => lz4::OneIOLz4::get_writer(output_file),
        _ => Ok(Box::new(BufWriter::new(output_file))),
    }
}
