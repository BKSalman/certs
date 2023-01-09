#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::{fs, sync::Arc};

use certs::{add_fonts, generate_certificate, Record};
use eframe::{
    egui::{self, Button, Sense, Ui},
    emath::Align2,
    epaint::{Color32, Pos2, Rect, Rounding, Stroke, Vec2},
    App,
};
use egui_extras::{Column, RetainedImage, TableBuilder};
use native_dialog::FileDialog;
use rand::{distributions::Standard, prelude::*};
use rayon::prelude::*;
use skia_safe::Point;

fn main() {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Certificates app",
        native_options,
        Box::new(|cc| Box::new(CertApp::new(cc))),
    );
}

struct CertApp {
    records: Vec<Record>,
    window_open: bool,
    image: Option<RetainedImage>,
    current_rect: usize,
    rects: [(TextRect, Color32); 3],
    template: Arc<Vec<u8>>,
}

impl Default for CertApp {
    fn default() -> Self {
        let mut rng = rand::thread_rng();
        Self {
            image: None,
            records: Vec::default(),
            window_open: false,
            current_rect: 0,
            rects: [
                (TextRect::default(), rng.gen::<Wrapper<Color32>>().0),
                (TextRect::default(), rng.gen::<Wrapper<Color32>>().0),
                (TextRect::default(), rng.gen::<Wrapper<Color32>>().0),
            ],
            template: Arc::default(),
        }
    }
}

#[derive(Clone)]
struct TextRect {
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

struct Wrapper<T>(T);

impl Distribution<Wrapper<Color32>> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Wrapper<Color32> {
        let (r, g, b) = rng.gen();
        Wrapper(Color32::from_rgb(r, g, b))
    }
}

impl CertApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self::default()
    }
    fn set_template(&mut self, template: Arc<Vec<u8>>) {
        self.template = template;
    }
    fn current_rect(&self) -> &TextRect {
        &self.rects[self.current_rect].0
    }
    fn table(&mut self, ui: &mut Ui) {
        let table = TableBuilder::new(ui)
            .striped(true)
            .cell_layout(egui::Layout::left_to_right(eframe::emath::Align::Center))
            .columns(Column::remainder().resizable(true), 3);

        table
            .header(20., |mut header| {
                header.col(|ui| {
                    ui.strong("ID");
                });
                header.col(|ui| {
                    ui.strong("Name");
                });
                header.col(|ui| {
                    ui.strong("Email");
                });
            })
            .body(|mut body| {
                for record in self.records.iter() {
                    body.row(18., |mut row| {
                        row.col(|ui| {
                            ui.label(record.id.clone());
                        });
                        row.col(|ui| {
                            let mut reshaped = arabic_reshaper::arabic_reshape(&record.name);
                            if !reshaped.is_ascii() {
                                reshaped = reshaped.chars().rev().collect();
                            }
                            ui.label(reshaped);
                        });
                        row.col(|ui| {
                            ui.label(record.email.clone());
                        });
                    });
                }
            });
    }

    fn import_csv(&mut self) -> anyhow::Result<()> {
        let current_dir = std::env::current_dir()?;

        let path = FileDialog::new()
            .set_location(&current_dir)
            .add_filter("CSV SpreadSheet", &["csv"])
            .show_open_single_file()?;

        if let Some(path) = path {
            let file = fs::read(path)?;
            println!("set file");
            let mut reader = csv::Reader::from_reader(&file[..]);
            self.records = reader
                .deserialize::<Record>()
                .map(|e| e.expect("proper csv entry"))
                .filter(|e| !e.id.is_empty() && !e.name.is_empty() && !e.email.is_empty())
                .collect::<Vec<Record>>();
            println!("save records");
        }

        Ok(())
    }

    fn parallel_certificates(&self) -> anyhow::Result<()> {
        {
            let records = self.records.clone();
            let width = (self.current_rect().p1.x - self.current_rect().p2.x).abs();
            let draw_pos =
                Rect::from_two_pos(self.current_rect().p1, self.current_rect().p2).left_top();
            let template = self.template.clone();

            std::thread::spawn(move || {
                records.par_iter().for_each(|record| {
                    generate_certificate(
                        &record,
                        Point::new(draw_pos.x, draw_pos.y) * 2.5,
                        width,
                        template.clone(),
                    );
                });
            });
        }
        Ok(())
    }

    fn pick_template(&mut self) -> anyhow::Result<()> {
        let current_dir = std::env::current_dir()?;

        let path = FileDialog::new()
            .set_location(&current_dir)
            .add_filter("Template Image", &["jpg", "png", "jpeg"])
            .show_open_single_file()?;
        if let Some(path) = path {
            let image = fs::read(path)?;
            self.image = Some(
                RetainedImage::from_image_bytes("Template Image", &image).expect("retained image"),
            );
            self.set_template(Arc::new(image));
        }

        Ok(())
    }
}

impl App for CertApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_fonts(add_fonts());
        egui::Window::new("Draw Areas")
            .open(&mut self.window_open)
            .anchor(Align2::CENTER_CENTER, [0., 0.])
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                let Some(template) = &self.image else {
                    ui.label("choose a template");
                    return;
                };
                let current = &mut self.rects[self.current_rect].0;
                let current_color = &self.rects[self.current_rect].1;

                let image = egui::Image::new(
                    template.texture_id(ctx),
                    Vec2::new(template.size_vec2().x / 2.5, template.size_vec2().y / 2.5),
                )
                .sense(Sense::drag());
                let image_res = ui.add(image);

                // window.width - img.width will give us: right border + left border, we divide
                // by 2 to get a single border space
                let border_diff = (ctx.used_rect().width() - image.size().x) / 2.;
                // the vertical diff is: top border + bottom border + (other elements)
                // if we remove the two borders, we get the diff of the elements and title
                let title_diff = ctx.used_rect().height() - image.size().y - (border_diff * 2.0);
                let offset = Vec2::new(border_diff, border_diff + title_diff)
                    + ctx.used_rect().min.to_vec2();

                if image_res.drag_started() {
                    if let Some(position) = image_res.interact_pointer_pos() {
                        current.p1 = position - offset;
                    }
                }

                if let Some(position) = image_res.interact_pointer_pos() {
                    current.p2 = position - offset;
                }

                ui.painter().rect(
                    Rect {
                        max: (current.p1.max(current.p2) + offset),
                        min: (current.p1.min(current.p2) + offset),
                    },
                    Rounding::none(),
                    Color32::TRANSPARENT,
                    Stroke::new(3., *current_color),
                );
                ui.horizontal(|ui| {
                    if ui.button("id").clicked() {
                        self.current_rect = 0;
                    }
                    if ui.button("name").clicked() {
                        self.current_rect = 1;
                    }
                    if ui.button("email").clicked() {
                        self.current_rect = 2;
                    }
                });
            });

        egui::TopBottomPanel::bottom("BottomPanel").show(ctx, |ui| {
            ui.set_enabled(!self.window_open);
            ui.horizontal(|ui| {
                let button = ui.add_sized([20., 30.], Button::new("Import CSV"));
                if button.clicked() {
                    self.import_csv().expect("import csv");
                }
                let button = ui.add_sized([20., 30.], Button::new("Create"));
                if button.clicked() {
                    self.parallel_certificates().expect("certificates");
                }
                let button = ui.add_sized([20., 30.], Button::new("Send Email"));
                if button.clicked() {
                    println!("Send Email")
                }
                let button = ui.add_sized([20., 30.], Button::new("Open Window"));
                if button.clicked() {
                    self.window_open = true;
                }
                let button = ui.add_sized([20., 30.], Button::new("Choose Template"));
                if button.clicked() {
                    self.pick_template().expect("pick template");
                }
            });
            ui.set_min_size(Vec2::new(ui.available_height(), 20.));
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.set_enabled(!self.window_open);
            self.table(ui);
        });
    }
}
