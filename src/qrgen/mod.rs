use std::{env, path::PathBuf, sync::mpsc::Sender};

use image::{ImageReader, Luma};
use qrcode::{QrCode, types::QrError};
use rand::{RngExt, distr::Alphanumeric};
use show_image::{ImageInfo, ImageView, create_window};

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

    pub fn show(&self, s_should_stop: Sender<bool>) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(location) = &self.save_location {
            let img = ImageReader::open(location)?.decode()?.to_rgb8();

            let (width, height) = img.dimensions();
            let image_view = ImageView::new(ImageInfo::rgb8(width, height), img.as_raw());

            let window = create_window("QuickShare", Default::default())?;
            window.set_image("QuickShare", image_view)?;

            // close on escape
            for event in window.event_channel()? {
                match event {
                    show_image::event::WindowEvent::CloseRequested(c) => {
                        s_should_stop.send(true);
                        break;
                    }
                    show_image::event::WindowEvent::KeyboardInput(e) => {
                        if e.input.key_code == Some(show_image::event::VirtualKeyCode::Escape) {
                            s_should_stop.send(true);
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(())
    }
}
