#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use certs::TextRect;
use csv::StringRecord;
use itertools::Itertools;
use skia_safe::Point;
use std::{fs, sync::Arc};

use certs::{add_fonts, generate_certificate};
use eframe::{
    egui::{self, Button, RichText, Sense, Ui},
    emath::Align2,
    epaint::{Color32, Rect, Rounding, Stroke, Vec2},
    App,
};
use egui_extras::{Column, RetainedImage, TableBuilder};
use native_dialog::FileDialog;
use rand::{distributions::Standard, prelude::*};
use rayon::prelude::*;

fn main() {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "Certificates app",
        native_options,
        Box::new(|cc| Box::new(CertApp::new(cc))),
    );
}

struct CertApp {
    columns: StringRecord,
    records: Vec<StringRecord>,
    window_open: bool,
    image: Option<RetainedImage>,
    current_rect: usize,
    rects: Vec<(TextRect, Color32)>,
    template: Arc<Vec<u8>>,
}

impl Default for CertApp {
    fn default() -> Self {
        Self {
            columns: StringRecord::default(),
            records: Vec::default(),
            window_open: false,
            image: None,
            current_rect: 0,
            rects: Vec::default(),
            template: Arc::default(),
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
    fn table(&mut self, ui: &mut Ui) {
        let table = TableBuilder::new(ui)
            .striped(true)
            .cell_layout(egui::Layout::left_to_right(eframe::emath::Align::Center))
            .columns(Column::remainder().resizable(true), self.columns.len());

        table
            .header(20., |mut header| {
                for column in self.columns.iter() {
                    header.col(|ui| {
                        ui.strong(column.to_uppercase());
                    });
                }
            })
            .body(|mut body| {
                for record in self.records.iter() {
                    body.row(18., |mut row| {
                        for column in record {
                            row.col(|ui| {
                                if !column.is_ascii() {
                                    let reshaped = arabic_reshaper::arabic_reshape(&column);
                                    let reshaped = reshaped.chars().rev().collect::<String>();
                                    ui.label(reshaped);
                                } else {
                                    ui.label(column);
                                }
                            });
                        }
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

            self.columns = reader.headers()?.clone();

            self.records = reader
                .records()
                .map(|r| r.expect("csv entry"))
                .filter(|r| r.iter().find(|r| r.is_empty()).is_none())
                .collect_vec();

            let mut rng = rand::thread_rng();
            for _ in 0..self.columns.len() {
                self.rects
                    .push((TextRect::default(), rng.gen::<Wrapper<Color32>>().0))
            }
            println!("save records");
        }

        Ok(())
    }

    fn parallel_certificates(&self) -> anyhow::Result<()> {
        {
            let records = self.records.clone();
            let points = self
                .rects
                .iter()
                .map(|r| {
                    let points = r.0.min();
                    (
                        Point::new(points.p1.x, points.p1.y) * 2.5,
                        (points.p1.x - points.p2.x).abs() * 2.5,
                    )
                })
                .collect::<Vec<(Point, f32)>>();
            let template = self.template.clone();

            std::thread::spawn(move || {
                records.par_iter().for_each(move |record| {
                    generate_certificate(record, points.clone(), template.clone());
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
                let (current, current_color) = &mut self.rects[self.current_rect];

                ui.label(
                    RichText::new(format!("Column: {}", &self.columns[self.current_rect]))
                        .color(*current_color),
                );

                let image = egui::Image::new(
                    template.texture_id(ctx),
                    Vec2::new(template.size_vec2().x / 2.5, template.size_vec2().y / 2.5),
                )
                .sense(Sense::drag());
                let image_res = ui.add(image);

                let offset = image_res.rect.min.to_vec2();

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
                    for (i, column) in self.columns.iter().enumerate() {
                        if ui.button(column).clicked() {
                            self.current_rect = i;
                        }
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
                let button = ui.add_sized([20., 30.], Button::new("Edit Layout"));
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
