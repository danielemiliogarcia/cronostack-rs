//! Shared widgets, plus the icons and font embedded in the binary.

use std::sync::Arc;

use egui::{FontFamily, FontId, RichText, Stroke, TextureHandle, Vec2};

use crate::theme::Palette;

const DIGITAL_FONT: &str = "digital";

/// Decodes an embedded PNG. Panics only if a bundled asset is corrupt,
/// which would be a build problem, not a runtime one.
pub fn decode_png(bytes: &[u8]) -> egui::ColorImage {
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .expect("bundled PNG asset is valid")
        .to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw())
}

/// Button icons, uploaded to the GPU once at startup.
pub struct Icons {
    pub add: TextureHandle,
    pub close: TextureHandle,
    pub play: TextureHandle,
    pub stop: TextureHandle,
    pub reset: TextureHandle,
    pub countdown: TextureHandle,
    pub hide: TextureHandle,
}

impl Icons {
    pub fn load(ctx: &egui::Context) -> Self {
        let texture = |name: &str, png: &[u8]| {
            ctx.load_texture(name, decode_png(png), egui::TextureOptions::LINEAR)
        };
        Self {
            add: texture("add", include_bytes!("../assets/icons/add.png")),
            close: texture("close", include_bytes!("../assets/icons/close.png")),
            play: texture("play", include_bytes!("../assets/icons/play.png")),
            stop: texture("stop", include_bytes!("../assets/icons/stop.png")),
            reset: texture("reset", include_bytes!("../assets/icons/reset.png")),
            countdown: texture("countdown", include_bytes!("../assets/icons/countdown.png")),
            hide: texture("hide", include_bytes!("../assets/icons/hide.png")),
        }
    }
}

/// A button showing a 16×16 icon.
pub fn icon_button(texture: &TextureHandle) -> egui::Button<'static> {
    let image = egui::Image::new(egui::load::SizedTexture::new(
        texture.id(),
        Vec2::splat(16.0),
    ));
    egui::Button::image(image).min_size(Vec2::new(36.0, 24.0))
}

/// Registers the seven-segment font used by the displays.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        DIGITAL_FONT.to_owned(),
        Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fonts/digital.ttf"
        ))),
    );
    fonts.families.insert(
        FontFamily::Name(DIGITAL_FONT.into()),
        vec![DIGITAL_FONT.to_owned()],
    );
    ctx.set_fonts(fonts);
}

/// A bordered box with an optional small centered title.
pub fn group<R>(
    ui: &mut egui::Ui,
    title: Option<&str>,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::symmetric(6, 4))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            if let Some(title) = title {
                ui.vertical_centered(|ui| ui.label(RichText::new(title).size(10.0).weak()));
            }
            add_contents(ui)
        })
        .inner
}

/// The big LCD-style display used by the stopwatch and the countdown.
pub fn digital_display(ui: &mut egui::Ui, palette: &Palette, text: &str, color: egui::Color32) {
    egui::Frame::new()
        .fill(palette.display_bg)
        .stroke(Stroke::new(2.0, palette.display_border))
        .corner_radius(2)
        .inner_margin(egui::Margin::symmetric(4, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.vertical_centered(|ui| {
                let font = FontId::new(24.0, FontFamily::Name(DIGITAL_FONT.into()));
                ui.label(RichText::new(text).font(font).color(color));
            });
        });
}
