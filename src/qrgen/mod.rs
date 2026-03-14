use image::Luma;
use qrcode::{QrCode, types::QrError};
use show_image::{ImageInfo, ImageView, WindowOptions, create_window};
use std::sync::mpsc::Sender;

pub struct QrGen {
    code: QrCode,
}

impl QrGen {
    pub fn create(data: &str) -> Result<Self, QrError> {
        let code = QrCode::new(data.as_bytes())?;
        Ok(Self { code })
    }

    fn render_rgb(&self) -> image::RgbImage {
        let luma = self.code.render::<Luma<u8>>().build();
        image::DynamicImage::ImageLuma8(luma).to_rgb8()
    }

    pub fn show(
        &self,
        s_should_stop: Sender<bool>,
        r_new_address: std::sync::mpsc::Receiver<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let img = self.render_rgb();
        let (width, height) = img.dimensions();
        let window = create_window(
            "QuickShare",
            WindowOptions::default()
                .set_size([width, height])
                .set_preserve_aspect_ratio(true),
        )?;
        window.set_image(
            "QuickShare",
            ImageView::new(ImageInfo::rgb8(width, height), img.as_raw()),
        )?;

        // watch for new addresses and re-render from a background thread
        let window_clone = window.clone();
        std::thread::spawn(move || {
            while let Ok(new_address) = r_new_address.recv() {
                println!("{}", &new_address);

                let qr = match QrGen::create(&new_address) {
                    Ok(q) => q,
                    Err(_) => continue,
                };
                let img = qr.render_rgb();
                let (w, h) = img.dimensions();
                let view = ImageView::new(ImageInfo::rgb8(w, h), img.as_raw());
                window_clone.set_image("QuickShare", view).ok();
            }
        });

        // main event loop (blocks on main thread)
        for event in window.event_channel()? {
            let should_stop = match event {
                show_image::event::WindowEvent::CloseRequested(_) => true,
                show_image::event::WindowEvent::KeyboardInput(e)
                    if e.input.key_code == Some(show_image::event::VirtualKeyCode::Escape) =>
                {
                    true
                }
                _ => false,
            };
            if should_stop {
                let _ = s_should_stop.send(true);
                break;
            }
        }
        Ok(())
    }
}
