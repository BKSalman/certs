use csv::StringRecord;
use eframe::egui::{FontData, FontDefinitions};
use eframe::epaint::{Color32, FontFamily, Pos2};
use lettre::message::header::ContentType;
use lettre::message::{Attachment, MultiPart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use rand::{distributions::Standard, prelude::*};
use serde::{Deserialize, Serialize};
use skia_safe::textlayout::{FontCollection, ParagraphBuilder, ParagraphStyle, TextStyle};
use skia_safe::{icu, Canvas, Data, EncodedImageFormat, FontMgr, Image, Paint, Point, Surface};
use std::collections::HashMap;
use std::fs;
use std::sync::Arc;

pub type Record = HashMap<String, String>;

pub struct Wrapper<T>(pub T);

impl Distribution<Wrapper<Color32>> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Wrapper<Color32> {
        let (r, g, b) = rng.gen();
        Wrapper(Color32::from_rgb(r, g, b))
    }
}

#[derive(Default, Clone, Debug, Deserialize, Serialize)]
pub struct Config {
    pub email: EmailCreds,
}

#[derive(Default, Clone, Debug, Deserialize, Serialize)]
pub struct EmailCreds {
    pub username: String,
    pub password: String,
}

#[derive(Clone)]
pub struct TextRect {
    pub p1: Pos2,
    pub p2: Pos2,
}

impl Default for TextRect {
    fn default() -> Self {
        Self {
            p1: Pos2::default(),
            p2: Pos2::default(),
        }
    }
}

impl TextRect {
    pub fn min(&self) -> Self {
        Self {
            p1: self.p1.min(self.p2),
            p2: self.p1.max(self.p2),
        }
    }
}

pub fn send_email(email_creds: EmailCreds, filename: &str, to: &str) -> anyhow::Result<()> {
    let attachment = Attachment::new(String::from("Certificate.png")).body(
        fs::read(format!("output/{}", filename)).expect("Read file"),
        ContentType::parse("image/png").expect("Failed to get MIME Type"),
    );

    let email = Message::builder()
        .from(email_creds.username.parse().unwrap())
        .to(to.parse().unwrap())
        .subject("شهادة حضور")
        .multipart(MultiPart::alternative().multipart(MultiPart::mixed().singlepart(attachment)))
        .expect("Email");

    let creds = Credentials::new(email_creds.username, email_creds.password);

    let mailer = SmtpTransport::relay("smtp.gmail.com")
        .unwrap()
        .credentials(creds)
        .build();

    mailer.send(&email)?;

    Ok(())
}

pub fn generate_certificate(
    record: &StringRecord,
    points: Vec<(Point, f32)>,
    template: Arc<Vec<u8>>,
    filename: &str,
    font_size: f32,
) {
    let data = Data::new_copy(&template);
    let image = Image::from_encoded(data).unwrap();
    let mut surface = Surface::new_raster_n32_premul(image.dimensions()).unwrap();
    let mut canvas = surface.canvas();
    canvas.draw_image(image, Point::new(0., 0.), Some(&Paint::default()));
    for (field, point) in record.iter().zip(points) {
        if point.0.is_zero() {
            println!("skipping {field}");
            continue;
        }

        let width = point.1;
        draw_text(&mut canvas, field, point.0, width, font_size);
    }
    save_as(&mut surface, &filename);
    println!("saved!");
}

fn draw_text(canvas: &mut Canvas, text: &str, position: Point, width: f32, font_size: f32) {
    icu::init();

    let mut font_collection = FontCollection::new();
    font_collection.set_default_font_manager(FontMgr::new(), None);

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_text_align(skia_safe::textlayout::TextAlign::Right);
    // paragraph_style.set_text_direction(skia_safe::textlayout::TextDirection::RTL);

    let mut text_style = TextStyle::new();
    text_style
        .set_font_families(&["Arial"])
        .set_font_size(font_size)
        .set_foreground_color(Paint::default());

    let mut paragraph_builder = ParagraphBuilder::new(&paragraph_style, font_collection);
    paragraph_builder.push_style(&text_style).add_text(text);
    let mut paragraph = paragraph_builder.build();
    paragraph.layout(width);
    paragraph.paint(canvas, position);
}

fn save_as(surface: &mut Surface, filename: &str) {
    let image = surface.image_snapshot();
    let data = image.encode_to_data(EncodedImageFormat::PNG).unwrap();
    match fs::create_dir_all("output") {
        Err(e) => match e.kind() {
            std::io::ErrorKind::AlreadyExists => {
                println!("dir already exists: {}", e);
            }
            std::io::ErrorKind::PermissionDenied => {
                // send to frontend somehow
                panic!("{e}")
            }
            _ => {
                panic!("{e}")
            }
        },
        _ => {}
    }
    fs::write(format!("output/{filename}"), data.as_bytes()).expect("failed to write to file");
}

pub fn add_fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        String::from("Arial"),
        FontData::from_static(include_bytes!("../assets/fonts/arial.ttf")),
    );

    fonts
        .families
        .get_mut(&FontFamily::Proportional)
        .unwrap()
        .push("Arial".to_owned());

    fonts
}

pub fn fix_text(text: &str) -> String {
    if !text.is_ascii() {
        return arabic_reshaper::arabic_reshape(text)
            .chars()
            .rev()
            .collect();
    }

    text.to_string()
}
