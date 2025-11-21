pub mod environment;
pub mod paths;
pub mod terminal;

pub use environment::get_claude_dir;
pub use paths::{
    decode_and_validate_path, decode_path, encode_path, format_path_with_tilde,
    validate_decoded_path, validate_file_size, validate_path_not_symlink,
};
pub use terminal::strip_ansi_codes;
