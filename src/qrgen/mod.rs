use std::{env, net::UdpSocket, path::PathBuf};

use image::Luma;
use qrcode::{QrCode, types::QrError};
use rand::{RngExt, distr::Alphanumeric};

pub struct QrGen {
    code: QrCode,
    save_location: Option<PathBuf>,
}

impl QrGen {
    pub fn create(data: &str) -> Result<Self, QrError> {
        let code = QrCode::new(data.as_bytes());
        match code {
            Ok(code) => Ok(Self {
                code: code,
                save_location: None,
            }),
            Err(e) => Err(e),
        }
    }

    pub fn generate_image(&mut self) {
        let image = self.code.render::<Luma<u8>>().build();

        let mut qr_location = env::temp_dir();

        let file_name: String = rand::rng()
            .sample_iter(&Alphanumeric)
            .take(12)
            .map(char::from)
            .collect();

        let file_name = format!("{}.png", file_name);

        qr_location.push(file_name);

        image.save(&qr_location).unwrap();

        self.save_location = Some(qr_location);
    }

    pub fn show(&self) {
        if let Some(location) = &self.save_location {
            win_open::that(location).expect("Unable to open qr image!");
        }
    }
}
