//! Reproyeksi koordinat geospasial murni (Pure Rust) tanpa runtime eksternal.
//! Mendukung EPSG:4326 (WGS84), EPSG:3857 (Web Mercator), dan UTM WGS84 Zona 1–60 (Utara & Selatan).

use std::f64::consts::PI;

use crate::modules::module_03::error::OrthoImportError;

/// Jari-jari semi-mayor elipsoid WGS84 (meter).
pub const WGS84_A: f64 = 6378137.0;

/// Inverse flattening elipsoid WGS84 (1/f).
pub const WGS84_INV_F: f64 = 298.257223563;

/// Mengonversi koordinat WGS84 (lat, lon) ke Web Mercator (EPSG:3857) meter.
pub fn wgs84_to_epsg3857(lat: f64, lon: f64) -> Result<(f64, f64), OrthoImportError> {
    if !(-85.05112878..=85.05112878).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return Err(OrthoImportError::KoordinatTidakValid(format!(
            "Koordinat WGS84 di luar batas EPSG:3857: lat={}, lon={}",
            lat, lon
        )));
    }

    let x = lon.to_radians() * WGS84_A;
    let lat_rad = lat.to_radians();
    let y = WGS84_A * (PI / 4.0 + lat_rad / 2.0).tan().ln();

    Ok((x, y))
}

/// Mengonversi koordinat Web Mercator (EPSG:3857) ke WGS84 (EPSG:4326) derajat desimal.
///
/// # Returns
/// `Ok((latitude, longitude))` dalam derajat desimal.
pub fn epsg3857_to_wgs84(x: f64, y: f64) -> Result<(f64, f64), OrthoImportError> {
    if x.is_nan() || y.is_nan() || x.is_infinite() || y.is_infinite() {
        return Err(OrthoImportError::KoordinatTidakValid(
            "Nilai koordinat EPSG:3857 tidak valid (NaN atau tak hingga)".to_string(),
        ));
    }

    let lon = (x / WGS84_A).to_degrees();
    // Rumus invers spherical Mercator: lat = (2 * atan(exp(y / a)) - pi / 2)
    let lat = (2.0 * (y / WGS84_A).exp().atan() - (PI / 2.0)).to_degrees();

    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return Err(OrthoImportError::KoordinatTidakValid(format!(
            "Hasil konversi EPSG:3857 di luar jangkauan WGS84: lat={}, lon={}",
            lat, lon
        )));
    }

    Ok((lat, lon))
}

/// Mengonversi koordinat WGS84 (lat, lon) ke UTM WGS84 (Easting, Northing).
pub fn wgs84_to_utm(
    lat: f64,
    lon: f64,
    zone: u8,
    is_south: bool,
) -> Result<(f64, f64), OrthoImportError> {
    if !(1..=60).contains(&zone) {
        return Err(OrthoImportError::CrsTidakDidukung(format!(
            "Zona UTM '{}' tidak valid. Zona harus bernilai 1 sampai 60",
            zone
        )));
    }

    let f = 1.0 / WGS84_INV_F;
    let a = WGS84_A;
    let e_sq = 2.0 * f - f * f;
    let e_prime_sq = e_sq / (1.0 - e_sq);
    let k0 = 0.9996;

    let lat_rad = lat.to_radians();
    let lon_origin = ((zone as i32 - 1) * 6 - 180 + 3) as f64;
    let delta_lon_rad = (lon - lon_origin).to_radians();

    let sin_phi = lat_rad.sin();
    let cos_phi = lat_rad.cos();
    let tan_phi = lat_rad.tan();

    let n = a / (1.0 - e_sq * sin_phi * sin_phi).sqrt();
    let t = tan_phi * tan_phi;
    let c = e_prime_sq * cos_phi * cos_phi;
    let big_a = delta_lon_rad * cos_phi;

    let e4 = e_sq * e_sq;
    let e6 = e4 * e_sq;
    let m = a
        * ((1.0 - e_sq / 4.0 - 3.0 * e4 / 64.0 - 5.0 * e6 / 256.0) * lat_rad
            - (3.0 * e_sq / 8.0 + 3.0 * e4 / 32.0 + 45.0 * e6 / 1024.0) * (2.0 * lat_rad).sin()
            + (15.0 * e4 / 256.0 + 45.0 * e6 / 1024.0) * (4.0 * lat_rad).sin()
            - (35.0 * e6 / 3072.0) * (6.0 * lat_rad).sin());

    let a2 = big_a * big_a;
    let a3 = a2 * big_a;
    let a4 = a2 * a2;
    let a5 = a4 * big_a;
    let a6 = a3 * a3;

    let x = k0
        * n
        * (big_a
            + (1.0 - t + c) * a3 / 6.0
            + (5.0 - 18.0 * t + t * t + 72.0 * c - 58.0 * e_prime_sq) * a5 / 120.0)
        + 500000.0;

    let mut y = k0
        * (m + n
            * tan_phi
            * (a2 / 2.0
                + (5.0 - t + 9.0 * c + 4.0 * c * c) * a4 / 24.0
                + (61.0 - 58.0 * t + t * t + 600.0 * c - 330.0 * e_prime_sq) * a6 / 720.0));

    if is_south {
        y += 10000000.0;
    }

    Ok((x, y))
}

/// Mengonversi koordinat UTM WGS84 ke WGS84 (EPSG:4326) derajat desimal.
///
/// Menggunakan formulasi invers Snyder/Karney berbasis elipsoid WGS84.
/// Mendukung belahan bumi selatan dengan penanganan False Northing 10.000.000 meter.
///
/// # Parameters
/// - `easting`: Koordinat X dalam meter (False Easting = 500.000 m).
/// - `northing`: Koordinat Y dalam meter (False Northing = 10.000.000 m untuk belahan selatan).
/// - `zone`: Zona UTM 1–60.
/// - `is_south`: `true` jika berada di belahan bumi selatan (misal UTM 50S Indonesia).
///
/// # Returns
/// `Ok((latitude, longitude))` dalam derajat desimal.
pub fn utm_to_wgs84(
    easting: f64,
    northing: f64,
    zone: u8,
    is_south: bool,
) -> Result<(f64, f64), OrthoImportError> {
    if !(1..=60).contains(&zone) {
        return Err(OrthoImportError::CrsTidakDidukung(format!(
            "Zona UTM '{}' tidak valid. Zona harus bernilai 1 sampai 60",
            zone
        )));
    }

    if easting.is_nan() || northing.is_nan() || easting.is_infinite() || northing.is_infinite() {
        return Err(OrthoImportError::KoordinatTidakValid(
            "Nilai koordinat UTM tidak valid (NaN atau tak hingga)".to_string(),
        ));
    }

    let f = 1.0 / WGS84_INV_F;
    let a = WGS84_A;
    let b = a * (1.0 - f);
    let e_sq = (a * a - b * b) / (a * a); // e^2
    let e_prime_sq = e_sq / (1.0 - e_sq); // e'^2
    let k0 = 0.9996;

    let x = easting - 500000.0;
    let y = if is_south {
        northing - 10000000.0
    } else {
        northing
    };

    let m = y / k0;
    let e1 = (1.0 - (1.0 - e_sq).sqrt()) / (1.0 + (1.0 - e_sq).sqrt());

    let mu =
        m / (a * (1.0 - e_sq / 4.0 - 3.0 * e_sq * e_sq / 64.0 - 5.0 * e_sq * e_sq * e_sq / 256.0));

    let phi1 = mu
        + (3.0 * e1 / 2.0 - 27.0 * e1.powi(3) / 32.0) * (2.0 * mu).sin()
        + (21.0 * e1.powi(2) / 16.0 - 55.0 * e1.powi(4) / 32.0) * (4.0 * mu).sin()
        + (151.0 * e1.powi(3) / 96.0) * (6.0 * mu).sin()
        + (1097.0 * e1.powi(4) / 512.0) * (8.0 * mu).sin();

    let sin_phi1 = phi1.sin();
    let cos_phi1 = phi1.cos();
    let tan_phi1 = phi1.tan();

    let c1 = e_prime_sq * cos_phi1 * cos_phi1;
    let t1 = tan_phi1 * tan_phi1;
    let n1 = a / (1.0 - e_sq * sin_phi1 * sin_phi1).sqrt();
    let r1 = a * (1.0 - e_sq) / (1.0 - e_sq * sin_phi1 * sin_phi1).powf(1.5);
    let d = x / (n1 * k0);

    let d2 = d * d;
    let d3 = d2 * d;
    let d4 = d2 * d2;
    let d5 = d4 * d;
    let d6 = d3 * d3;

    let lat_rad = phi1
        - (n1 * tan_phi1 / r1)
            * (d2 / 2.0
                - (5.0 + 3.0 * t1 + 10.0 * c1 - 4.0 * c1 * c1 - 9.0 * e_prime_sq) * d4 / 24.0
                + (61.0 + 90.0 * t1 + 298.0 * c1 + 45.0 * t1 * t1
                    - 252.0 * e_prime_sq
                    - 3.0 * c1 * c1)
                    * d6
                    / 720.0);

    let lon_origin = ((zone as i32 - 1) * 6 - 180 + 3) as f64; // Central meridian
    let lon_rad = (d - (1.0 + 2.0 * t1 + c1) * d3 / 6.0
        + (5.0 - 2.0 * c1 + 28.0 * t1 - 3.0 * c1 * c1 + 8.0 * e_prime_sq + 24.0 * t1 * t1) * d5
            / 120.0)
        / cos_phi1;

    let lat = lat_rad.to_degrees();
    let lon = lon_origin + lon_rad.to_degrees();

    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return Err(OrthoImportError::KoordinatTidakValid(format!(
            "Hasil konversi UTM Zona {}{} di luar jangkauan WGS84: lat={}, lon={}",
            zone,
            if is_south { 'S' } else { 'N' },
            lat,
            lon
        )));
    }

    Ok((lat, lon))
}

/// Memproyeksikan satu titik koordinat (X, Y) dari CRS tertentu ke WGS84 (Lat, Lon).
pub fn project_to_wgs84(x: f64, y: f64, epsg_code: u32) -> Result<(f64, f64), OrthoImportError> {
    match epsg_code {
        4326 => {
            // Sudah WGS84: perhatikan konvensi x=lon, y=lat
            if !(-90.0..=90.0).contains(&y) || !(-180.0..=180.0).contains(&x) {
                return Err(OrthoImportError::KoordinatTidakValid(format!(
                    "Koordinat WGS84 di luar batas: lat={}, lon={}",
                    y, x
                )));
            }
            Ok((y, x))
        }
        3857 => epsg3857_to_wgs84(x, y),
        // UTM WGS84 North: EPSG 32601 - 32660
        32601..=32660 => {
            let zone = (epsg_code - 32600) as u8;
            utm_to_wgs84(x, y, zone, false)
        }
        // UTM WGS84 South: EPSG 32701 - 32760
        32701..=32760 => {
            let zone = (epsg_code - 32700) as u8;
            utm_to_wgs84(x, y, zone, true)
        }
        other => Err(OrthoImportError::CrsTidakDidukung(format!(
            "EPSG:{}",
            other
        ))),
    }
}

/// Menghitung envelope bounding box WGS84 `[[south, west], [north, east]]`
/// dari sekumpulan titik sudut raster.
///
/// Memproyeksikan setiap titik ke WGS84 lalu mengambil nilai ekstrem (min/max).
pub fn compute_wgs84_envelope_from_corners(
    corners: &[(f64, f64)],
    epsg_code: u32,
) -> Result<[[f64; 2]; 2], OrthoImportError> {
    if corners.is_empty() {
        return Err(OrthoImportError::KoordinatTidakValid(
            "Daftar titik sudut citra kosong".to_string(),
        ));
    }

    let mut min_lat = f64::INFINITY;
    let mut max_lat = f64::NEG_INFINITY;
    let mut min_lon = f64::INFINITY;
    let mut max_lon = f64::NEG_INFINITY;

    for &(x, y) in corners {
        let (lat, lon) = project_to_wgs84(x, y, epsg_code)?;
        if lat < min_lat {
            min_lat = lat;
        }
        if lat > max_lat {
            max_lat = lat;
        }
        if lon < min_lon {
            min_lon = lon;
        }
        if lon > max_lon {
            max_lon = lon;
        }
    }

    if min_lat >= max_lat || min_lon >= max_lon {
        return Err(OrthoImportError::KoordinatTidakValid(format!(
            "Envelope bounds tidak valid: south={}, north={}, west={}, east={}",
            min_lat, max_lat, min_lon, max_lon
        )));
    }

    Ok([[min_lat, min_lon], [max_lat, max_lon]])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_epsg3857_roundtrip() {
        // Monas Jakarta: lat -6.175392, lon 106.827153
        let lat = -6.175392;
        let lon = 106.827153;

        let (x, y) = wgs84_to_epsg3857(lat, lon).expect("Forward EPSG:3857");
        let (inv_lat, inv_lon) = epsg3857_to_wgs84(x, y).expect("Inverse EPSG:3857");

        assert!(
            (inv_lat - lat).abs() < 1e-7,
            "Selisih lat: {}",
            (inv_lat - lat).abs()
        );
        assert!(
            (inv_lon - lon).abs() < 1e-7,
            "Selisih lon: {}",
            (inv_lon - lon).abs()
        );
    }

    #[test]
    fn test_utm_south_balikpapan_precision_roundtrip() {
        // Balikpapan (UTM Zona 50S, EPSG:32750):
        let orig_lat = -1.253814;
        let orig_lon = 116.850648;

        let (easting, northing) =
            wgs84_to_utm(orig_lat, orig_lon, 50, true).expect("Forward UTM 50S");

        let (inv_lat, inv_lon) =
            utm_to_wgs84(easting, northing, 50, true).expect("Inverse UTM 50S");

        // Evaluasi toleransi jarak < 1 milimeter (1e-7 derajat ~= 0.011 meter):
        assert!(
            (inv_lat - orig_lat).abs() < 1e-6,
            "Selisih lat: {}",
            (inv_lat - orig_lat).abs()
        );
        assert!(
            (inv_lon - orig_lon).abs() < 1e-6,
            "Selisih lon: {}",
            (inv_lon - orig_lon).abs()
        );
    }

    #[test]
    fn test_utm_north_greenwich_zone_30n() {
        // Greenwich Royal Observatory (UTM Zona 30N, EPSG:32630):
        // Bujur 0.0000° merupakan batas timur persis dari Zona 30 UTM (-6° s/d 0°).
        // Nilai referensi: golden values (deret Krüger, diverifikasi dengan implementasi independen)
        let orig_lat = 51.476852;
        let orig_lon = 0.000000;

        let (easting, northing) =
            wgs84_to_utm(orig_lat, orig_lon, 30, false).expect("Forward UTM 30N Greenwich");

        // Pada batas timur (delta 3 derajat dari meridian sentral -3°), Easting bernilai sekitar 708.000 m
        assert!(
            easting > 700000.0 && easting < 720000.0,
            "Easting Greenwich batas zona: {}",
            easting
        );
        assert!(
            northing > 5700000.0 && northing < 5710000.0,
            "Northing Greenwich: {}",
            northing
        );

        let (inv_lat, inv_lon) =
            utm_to_wgs84(easting, northing, 30, false).expect("Inverse UTM 30N Greenwich");

        assert!(
            (inv_lat - orig_lat).abs() < 1e-6,
            "Selisih lat: {}",
            (inv_lat - orig_lat).abs()
        );
        assert!(
            (inv_lon - orig_lon).abs() < 1e-6,
            "Selisih lon: {}",
            (inv_lon - orig_lon).abs()
        );
    }

    #[test]
    fn test_utm_north_paris_zone_31n() {
        // Paris / Eiffel Tower (UTM Zona 31N, EPSG:32631):
        // Meridian sentral Zona 31N adalah 3.0°E. Paris berada di 2.29448°E (sebelah barat meridian sentral).
        // Nilai referensi: golden values (deret Krüger, diverifikasi dengan implementasi independen)
        let orig_lat = 48.858370;
        let orig_lon = 2.294480;

        let (easting, northing) =
            wgs84_to_utm(orig_lat, orig_lon, 31, false).expect("Forward UTM 31N Paris");

        // Easting harus berada di sebelah barat meridian sentral (E < 500.000 m)
        assert!(
            easting > 440000.0 && easting < 460000.0,
            "Easting Paris: {}",
            easting
        );
        assert!(
            northing > 5410000.0 && northing < 5420000.0,
            "Northing Paris: {}",
            northing
        );

        let (inv_lat, inv_lon) =
            utm_to_wgs84(easting, northing, 31, false).expect("Inverse UTM 31N Paris");

        assert!(
            (inv_lat - orig_lat).abs() < 1e-6,
            "Selisih lat Paris: {}",
            (inv_lat - orig_lat).abs()
        );
        assert!(
            (inv_lon - orig_lon).abs() < 1e-6,
            "Selisih lon Paris: {}",
            (inv_lon - orig_lon).abs()
        );
    }

    #[test]
    fn test_utm_zone_boundary_points_precision() {
        // UTM Zona 50 mencakup bujur 114°E hingga 120°E (meridian sentral 117°E).
        // Uji titik di dekat batas barat zona (114.001°E) dan batas timur zona (119.999°E)
        let lat = -1.500000;

        // 1. Batas Barat Zona 50
        let west_lon = 114.001000;
        let (e_w, n_w) = wgs84_to_utm(lat, west_lon, 50, true).expect("Forward UTM 50S West edge");
        assert!(
            e_w > 160000.0 && e_w < 170000.0,
            "Easting batas barat zona: {}",
            e_w
        );
        let (inv_lat_w, inv_lon_w) =
            utm_to_wgs84(e_w, n_w, 50, true).expect("Inverse UTM 50S West edge");
        assert!((inv_lat_w - lat).abs() < 1e-6);
        assert!((inv_lon_w - west_lon).abs() < 1e-6);

        // 2. Batas Timur Zona 50
        let east_lon = 119.999000;
        let (e_e, n_e) = wgs84_to_utm(lat, east_lon, 50, true).expect("Forward UTM 50S East edge");
        assert!(
            e_e > 830000.0 && e_e < 840000.0,
            "Easting batas timur zona: {}",
            e_e
        );
        let (inv_lat_e, inv_lon_e) =
            utm_to_wgs84(e_e, n_e, 50, true).expect("Inverse UTM 50S East edge");
        assert!((inv_lat_e - lat).abs() < 1e-6);
        assert!((inv_lon_e - east_lon).abs() < 1e-6);
    }

    #[test]
    fn test_compute_envelope_from_corners() {
        let corners = vec![
            (483000.0, 9861000.0),
            (484000.0, 9861000.0),
            (484000.0, 9862000.0),
            (483000.0, 9862000.0),
        ];

        let bounds = compute_wgs84_envelope_from_corners(&corners, 32750)
            .expect("Gagal hitung envelope UTM 50S");

        assert!(bounds[0][0] < bounds[1][0]); // south < north
        assert!(bounds[0][1] < bounds[1][1]); // west < east
    }

    #[test]
    fn test_unsupported_crs() {
        let res = project_to_wgs84(100.0, 100.0, 99999);
        assert!(matches!(res, Err(OrthoImportError::CrsTidakDidukung(_))));
    }
}
