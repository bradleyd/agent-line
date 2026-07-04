//! Standalone utility functions for common agent tasks.

pub mod command;
pub mod file;
pub mod http;
pub mod parse;

pub use command::{CmdOutput, run_cmd, run_cmd_in_dir};
pub use file::{
    append_file, create_dir, delete_file, file_exists, find_files, list_dir, read_file, write_file,
};
pub use http::{
    http_get, http_get_with_headers, http_post, http_post_json, http_post_json_with_headers,
    http_post_with_headers,
};
pub use parse::{extract_json, parse_lines, strip_code_fences};
