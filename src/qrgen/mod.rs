use image::Luma;
use qrcode::{QrCode, types::QrError};
use show_image::{ImageInfo, ImageView, WindowOptions, WindowProxy, create_window};
use std::sync::{
    Arc, Mutex,
    mpsc::{Receiver, Sender},
};

pub struct QrGen {
    code: Arc<Mutex<QrCode>>,
    s_stop: Sender<bool>,
    r_new_address: Option<Receiver<String>>,
    window: Option<WindowProxy>,
}

impl QrGen {
    pub fn create(
        data: &str,
        s_stop: Sender<bool>,
        r_new_address: Option<Receiver<String>>,
    ) -> Result<Self, QrError> {
        let code = Arc::new(Mutex::new(QrCode::new(data.as_bytes())?));

        Ok(Self {
            code: code,
            s_stop,
            r_new_address: r_new_address,
            window: None,
        })
    }
    fn render_rgb(&self) -> image::RgbImage {
        let code = self.code.lock().unwrap();
        let luma = code.render::<Luma<u8>>().build();
        image::DynamicImage::ImageLuma8(luma).to_rgb8()
    }

    // listen to regen requests
    fn listen(&mut self) {
        let window = self
            .window
            .as_ref()
            .expect("Call show() before listen()")
            .clone();
        let code = Arc::clone(&self.code);
        let receiver = self.r_new_address.take().expect("listen() already called");

        std::thread::spawn(move || {
            while let Ok(new_address) = receiver.recv() {
                *code.lock().unwrap() = QrCode::new(new_address.as_bytes()).unwrap();

                let luma = code.lock().unwrap().render::<Luma<u8>>().build();
                let img = image::DynamicImage::ImageLuma8(luma).to_rgb8();
                let (w, h) = img.dimensions();
                let view = ImageView::new(ImageInfo::rgb8(w, h), img.as_raw());
                window.set_image("QuickShare", view).ok();
            }
        });
    }
    pub fn show(&mut self) -> Result<(), Box<dyn std::error::Error>> {
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
        self.window = Some(window.clone());

        // listen for regen request
        self.listen();

        for event in window.event_channel().unwrap() {
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
                let _ = self.s_stop.send(true);
                break;
            }
        }

        Ok(())
    }
}
