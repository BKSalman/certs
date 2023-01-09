use eframe::egui::{FontData, FontDefinitions};
use eframe::epaint::FontFamily;
use serde::Deserialize;
use skia_safe::textlayout::{FontCollection, ParagraphBuilder, ParagraphStyle, TextStyle};
use skia_safe::{
    icu, Canvas, Data, EncodedImageFormat, Font, FontMgr, FontStyle, Image, Paint, Point, Surface,
    Typeface,
};
use std::fs;

pub const TEMPLATE: &[u8] = include_bytes!("../assets/template.jpg");

#[derive(Debug, Deserialize, Clone)]
pub struct Record {
    pub id: String,
    pub name: String,
    pub email: String,
}

pub fn generate_certificate(record: &Record, position: Point, width: f32) {
    let filename = format!("{}-{}", record.id, record.name);
    let data = Data::new_copy(TEMPLATE);
    let image = Image::from_encoded(data).unwrap();
    let mut surface = Surface::new_raster_n32_premul(image.dimensions()).unwrap();
    let mut canvas = surface.canvas();
    canvas.draw_image(image, Point::new(0., 0.), Some(&Paint::default()));
    draw_text(&mut canvas, &record.id, position, width);
    // draw_text(&mut canvas, &record.name, Point::new(600., 400.));
    // draw_text(&mut canvas, &record.email, Point::new(600., 400.));
    save_as(&mut surface, &filename);
    println!("saved!");
}

fn draw_text(canvas: &mut Canvas, text: &str, position: Point, width: f32) {
    icu::init();

    let mut font_collection = FontCollection::new();
    font_collection.set_default_font_manager(FontMgr::new(), None);

    let mut paragraph_style = ParagraphStyle::new();
    paragraph_style.set_text_align(skia_safe::textlayout::TextAlign::Right);

    let mut text_style = TextStyle::new();
    text_style
        .set_font_families(&["AlHor"])
        .set_font_size(40.)
        .set_foreground_color(Paint::default());

    let font = Font::new(
        Typeface::from_name("AlHor", FontStyle::default()).expect("typeface"),
        40.,
    );
    let measured = font.measure_str("someting", Some(&Paint::default()));

    println!("{measured:#?}");

    let mut paragraph_builder = ParagraphBuilder::new(&paragraph_style, font_collection);
    paragraph_builder.push_style(&text_style).add_text(text);
    let mut paragraph = paragraph_builder.build();
    println!("width: {}", width);
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
