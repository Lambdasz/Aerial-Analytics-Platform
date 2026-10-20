//! Test suite verifikasi data sampel terpusat Module 3 (Stage 6).

use aerial_analytics_platform_lib::map_controller::geometry::validate_polygon;
use aerial_analytics_platform_lib::map_controller::sample_data_commands::*;
use aerial_analytics_platform_lib::map_controller::sp_result::reader::get_dummy_spatial_layers;
use aerial_analytics_platform_lib::map_controller::types::aoi_type::AoiFeature;
use aerial_analytics_platform_lib::map_controller::types::sp_result_type::{
    LayerDisplayMode, SpatialGeometry,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

const KARIANGAU_MIN_LON: f64 = 116.855973;
const KARIANGAU_MAX_LON: f64 = 116.863203;
const KARIANGAU_MIN_LAT: f64 = -1.224312;
const KARIANGAU_MAX_LAT: f64 = -1.218629;

/// Implementasi murni SHA-256 (FIPS 180-4) tanpa dependensi eksternal.
fn sha256_digest(input: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    let bit_len = (input.len() as u64) * 8;
    let mut msg = input.to_vec();
    msg.push(0x80);
    while (msg.len() % 64) != 56 {
        msg.push(0x00);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in msg.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (i, part) in chunk.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes([part[0], part[1], part[2], part[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut h_val = h[7];

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h_val
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h_val = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(h_val);
    }

    h.iter().map(|val| format!("{:08x}", val)).collect()
}

#[derive(Deserialize)]
struct Manifest {
    files: HashMap<String, String>,
}

#[test]
fn test_d1_deserialize_all_sample_resources() {
    let markers = get_project_image_markers().expect("markers.json harus valid");
    assert_eq!(markers.len(), 200);

    let layers = get_initial_map_layers().expect("layers.json harus valid");
    assert_eq!(layers.len(), 5);

    let aoi_list = get_sample_aoi().expect("aoi.json harus valid");
    assert!(!aoi_list.is_empty());

    let annotations = get_sample_annotations().expect("annotations.json harus valid");
    assert!(!annotations.is_empty());

    let spatial_layers = get_dummy_spatial_layers().expect("spatial_results.json harus valid");
    assert_eq!(spatial_layers.len(), 6);
}

#[test]
fn test_d2_manifest_sha256_integrity() {
    let sample_dir = Path::new("resources/sample-data/module3");
    let manifest_path = sample_dir.join("manifest.json");
    let manifest_content = fs::read_to_string(&manifest_path).expect("manifest.json harus ada");
    let manifest: Manifest =
        serde_json::from_str(&manifest_content).expect("manifest.json harus valid");

    for (rel_path, expected_hash) in &manifest.files {
        let file_path = sample_dir.join(rel_path);
        let bytes = fs::read(&file_path)
            .unwrap_or_else(|_| panic!("File {} harus dapat dibaca", file_path.display()));
        let actual_hash = sha256_digest(&bytes);

        assert_eq!(
            &actual_hash, expected_hash,
            "Hash SHA-256 tidak cocok untuk file: {}",
            rel_path
        );
    }
}

#[test]
fn test_d3_geospatial_bounds_consistency() {
    let markers = get_project_image_markers().unwrap();
    for marker in &markers {
        assert!(
            (KARIANGAU_MIN_LAT..=KARIANGAU_MAX_LAT).contains(&marker.latitude),
            "Marker {} lat {} di luar batas Kariangau",
            marker.image_id,
            marker.latitude
        );
        assert!(
            (KARIANGAU_MIN_LON..=KARIANGAU_MAX_LON).contains(&marker.longitude),
            "Marker {} lon {} di luar batas Kariangau",
            marker.image_id,
            marker.longitude
        );
        assert_eq!(marker.altitude_agl, 120.0);
    }

    let aoi_list = get_sample_aoi().unwrap();
    for aoi in &aoi_list {
        validate_polygon(&aoi.geometry.coordinates).expect("AOI harus valid");
        for ring in &aoi.geometry.coordinates {
            for &[lon, lat] in ring {
                assert!(
                    (KARIANGAU_MIN_LON..=KARIANGAU_MAX_LON).contains(&lon),
                    "AOI lon {} di luar bounds",
                    lon
                );
                assert!(
                    (KARIANGAU_MIN_LAT..=KARIANGAU_MAX_LAT).contains(&lat),
                    "AOI lat {} di luar bounds",
                    lat
                );
            }
        }
    }

    let annotations = get_sample_annotations().unwrap();
    for anno in &annotations {
        if let SpatialGeometry::Point { coordinates } = &anno.geometry {
            let (lon, lat) = (coordinates[0], coordinates[1]);
            assert!(
                (KARIANGAU_MIN_LON..=KARIANGAU_MAX_LON).contains(&lon),
                "Annotation lon di luar bounds"
            );
            assert!(
                (KARIANGAU_MIN_LAT..=KARIANGAU_MAX_LAT).contains(&lat),
                "Annotation lat di luar bounds"
            );
        }
    }

    let spatial_layers = get_dummy_spatial_layers().unwrap();
    for cmd in &spatial_layers {
        if let LayerDisplayMode::ImageOverlay { bounds, .. } = &cmd.payload.display_preference {
            let [[s, w], [n, e]] = bounds;
            assert!(*w >= KARIANGAU_MIN_LON - 0.001);
            assert!(*e <= KARIANGAU_MAX_LON + 0.001);
            assert!(*s >= KARIANGAU_MIN_LAT - 0.001);
            assert!(*n <= KARIANGAU_MAX_LAT + 0.001);
        }
    }
}

#[test]
fn test_d5_determinism_sample_data_commands() {
    let markers1 = get_project_image_markers().unwrap();
    let markers2 = get_project_image_markers().unwrap();
    assert_eq!(markers1, markers2);

    let layers1 = get_initial_map_layers().unwrap();
    let layers2 = get_initial_map_layers().unwrap();
    assert_eq!(layers1, layers2);

    let aoi1 = get_sample_aoi().unwrap();
    let aoi2 = get_sample_aoi().unwrap();
    assert_eq!(aoi1, aoi2);

    let annotations1 = get_sample_annotations().unwrap();
    let annotations2 = get_sample_annotations().unwrap();
    assert_eq!(annotations1, annotations2);

    let spatial1 = get_dummy_spatial_layers().unwrap();
    let spatial2 = get_dummy_spatial_layers().unwrap();
    assert_eq!(spatial1, spatial2);
}

#[test]
fn test_d6_validate_example_docs() {
    let aoi_example_str =
        include_str!("../../src/modules/module-03/docs/examples/aoi.example.geojson");
    let aoi_feature: AoiFeature = serde_json::from_str(aoi_example_str)
        .expect("aoi.example.geojson harus parse ke AoiFeature");
    validate_polygon(&aoi_feature.geometry.coordinates).expect("aoi.example.geojson harus valid");

    let aoi_invalid_str =
        include_str!("../../src/modules/module-03/docs/examples/aoi.invalid.example.geojson");
    let aoi_invalid_feature: AoiFeature = serde_json::from_str(aoi_invalid_str)
        .expect("aoi.invalid.example.geojson harus parse ke AoiFeature");
    assert!(
        validate_polygon(&aoi_invalid_feature.geometry.coordinates).is_err(),
        "aoi.invalid.example.geojson wajib gagal validasi poligon"
    );
}
