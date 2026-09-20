use thiserror::Error;

#[derive(Error, Debug)]
pub enum MapControllerError {
    #[error("geometri tidak valid: {0}")]
    InvalidGeometry(String),
    #[error("gagal I/O: {0}")]
    Io(#[from] std::io::Error),
}

impl From<MapControllerError> for CommandError {
    fn from(err: MapControllerError) -> Self {
        log::error!("[map_controller] {err}"); 
        CommandError {
            code: err.code(), 
            message: err.to_string(),
        }
    }
}