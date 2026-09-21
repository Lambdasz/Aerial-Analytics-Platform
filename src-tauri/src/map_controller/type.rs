#![allow(dead_code)]

//! Type definitions for the Map Controller module (Module 3).
//!
//! Contains DTOs and geometry types for spatial results used by
//! the map controller commands.

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Global_input_sp {}

#[derive(Debug, Serialize, Deserialize)]
pub struct Display_data_sp {}

#[derive(Debug, Serialize, Deserialize)]
pub struct Sp_result {}

#[derive(Debug, Serialize, Deserialize)]
pub struct Sp_r_source {
    pub source: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Sp_r_geometry {
    #[serde(rename = "type")]
    pub g_type: String,
    pub coordinates: Vec<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Sp_r_properties_tree {
    pub tree_id: String,
    pub confidence: f64,
    pub height_est_m: f64,
}

pub struct Sp_r_properties_vegetation {
    pub vegetation_id: String,
    pub confidence: f64,
}

pub enum Sp_r_properties {
    Tree(Sp_r_properties_tree),
    Building(Sp_r_properties_building),
}
