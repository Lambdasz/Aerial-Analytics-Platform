//! Modul penanganan hasil spasial analitik.

pub mod dispatcher;
pub mod reader;
pub mod validation;

pub use dispatcher::*;
pub use reader::*;

pub use crate::modules::module_03::types::sp_result_type::SPATIAL_RESULT_EVENT;
pub mod dummy {
    pub use super::reader::get_dummy_spatial_layers;
}
