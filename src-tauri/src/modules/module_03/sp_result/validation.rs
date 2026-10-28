//! Validasi payload hasil spasial analitik.

use crate::modules::module_03::error::MapControllerError;
use crate::modules::module_03::types::sp_result_type::{
    LayerDisplayMode, LayerPayload, SpatialResult,
};

impl SpatialResult {
    /// Mengeksekusi validasi pada satu objek hasil spasial.
    pub fn validate(&self) -> Result<(), MapControllerError> {
        self.geometry.validate()?;
        Ok(())
    }
}

impl LayerPayload {
    /// Memvalidasi seluruh konfigurasi dan data sebelum diteruskan ke frontend.
    pub fn validate(&self) -> Result<(), MapControllerError> {
        if let LayerDisplayMode::ImageOverlay { opacity, .. } = &self.display_preference {
            if *opacity < 0.0 || *opacity > 1.0 {
                return Err(MapControllerError::InvalidGeometry(
                    "Opacity ImageOverlay harus di antara 0.0 hingga 1.0".to_string(),
                ));
            }
        }

        for result in &self.spatial_results {
            result.validate()?;
        }
        Ok(())
    }
}
