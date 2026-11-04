//! Modul penanganan hasil pengukuran spasial (sp_result) Module 3.

pub mod sp_result_constants;
pub mod sp_result_dispatcher_io;
pub mod sp_result_ipc_cmd;
pub mod sp_result_logic;
pub mod sp_result_patch_io;
pub mod sp_result_quarantine_io;
pub mod sp_result_reader_io;
pub mod sp_result_storage_io;
pub mod sp_result_struct;
pub mod sp_result_validation_logic;
pub mod sp_result_writer_io;

// Alias kompatibilitas mundur
pub use sp_result_constants as constants;
pub use sp_result_dispatcher_io as dispatcher;
pub use sp_result_ipc_cmd as commands;
pub use sp_result_logic as pure;
pub use sp_result_patch_io as patch;
pub use sp_result_quarantine_io as quarantine;
pub use sp_result_reader_io as reader;
pub use sp_result_storage_io as store;
pub use sp_result_struct as types;
pub use sp_result_validation_logic as validation;
pub use sp_result_writer_io as writer;

pub use sp_result_constants::*;
pub use sp_result_dispatcher_io::*;
pub use sp_result_ipc_cmd::*;
pub use sp_result_logic::*;
pub use sp_result_struct::*;

pub use crate::modules::module_03::types::sp_result_type::SPATIAL_RESULT_EVENT;
