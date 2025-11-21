pub mod environment;
pub mod paths;

pub use environment::get_claude_dir;
pub use paths::{
    decode_and_validate_path, decode_path, encode_path, format_path_with_tilde,
    validate_decoded_path, validate_file_size,
};
