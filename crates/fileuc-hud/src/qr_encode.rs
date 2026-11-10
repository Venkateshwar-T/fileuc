//! High-quality QR Code encoder wrapper based on Project Nayuki's ISO/IEC 18004 engine.

use crate::qrcodegen::{QrCode as NayukiQr, QrCodeEcc};

#[derive(Clone, Debug, PartialEq)]
pub struct QrCode {
    pub size: usize,
    pub modules: Vec<Vec<bool>>,
}

impl QrCode {
    pub fn encode_text(text: &str) -> Result<Self, String> {
        let qr = NayukiQr::encode_text(text, QrCodeEcc::Medium)
            .map_err(|e| e.to_string())?;
        let size = qr.size() as usize;
        let mut modules = vec![vec![false; size]; size];
        for y in 0..size {
            for x in 0..size {
                modules[y][x] = qr.get_module(x as i32, y as i32);
            }
        }
        Ok(Self { size, modules })
    }
}
