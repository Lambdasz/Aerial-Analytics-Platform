//! Tipe data yang dipakai di seluruh `reporting`.
//!
//! Dipecah jadi dua submodul mengikuti aturan skala di `RULES_REPORTING.md`
//! (folder `types/` dipakai begitu jumlah struct sudah lebih dari 5-6):
//!
//! - [`input`] — representasi data yang **masuk** dari Module 9 dan Module 10.
//! - [`output`] — model Report + Export milik Module 11 sendiri.

pub mod input;
pub mod output;
