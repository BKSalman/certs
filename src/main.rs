#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use certs::{fix_text, send_email, Config, EmailCreds, TextRect, Wrapper};
use csv::StringRecord;
use itertools::Itertools;
use rand::Rng;
use skia_safe::Point;
use std::{fs, sync::Arc, thread::JoinHandle};

use certs::{add_fonts, generate_certificate};
use eframe::{
    egui::{self, Button, RichText, Sense, Ui},
    emath::Align2,
    epaint::{Color32, Rect, Rounding, Stroke, Vec2},
    App,
};
use egui_extras::{Column, RetainedImage, TableBuilder};
use native_dialog::FileDialog;
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
    template_window_open: bool,
    email_window_open: bool,
    send_email_window_open: bool,
    certificates_window_open: bool,
    status: String,
    image: Option<RetainedImage>,
    current_rect: usize,
    rects: Vec<(TextRect, Color32)>,
    template: Arc<Vec<u8>>,
    config: Config,
    current_email_creds: EmailCreds,
    t_handle: Option<JoinHandle<()>>,
    font_size: f32,
}

impl Default for CertApp {
    fn default() -> Self {
        let config_dir = dirs::config_dir().expect("config directory").join("certs/");

        fs::create_dir_all(&config_dir).expect("create config dir");

        #[cfg(not(feature = "baba"))]
        let config_str = match fs::read_to_string(config_dir.join("config.toml")) {
            Ok(file) => file,
            Err(e) => {
                println!("{e}");
                let new_config = toml::to_string(&Config::default()).expect("Config to string");
                fs::write(config_dir.join("config.toml"), &new_config)
                    .expect("create new config file");

                new_config
            }
        };

        #[cfg(feature = "baba")]
        let config_str = include_str!("../baba.toml");

        let config = toml::from_str::<Config>(&config_str).expect("deserialize config");

        Self {
            columns: StringRecord::default(),
            records: Vec::default(),
            template_window_open: false,
            email_window_open: false,
            send_email_window_open: false,
            certificates_window_open: false,
            status: String::new(),
            image: None,
            current_rect: 0,
            rects: Vec::default(),
            template: Arc::default(),
            config: config.clone(),
            current_email_creds: config.email,
            t_handle: None,
            font_size: 40.,
        }
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
                        ui.strong(fix_text(&column.to_uppercase()));
                    });
                }
            })
            .body(|mut body| {
                for record in self.records.iter() {
                    body.row(18., |mut row| {
                        for column in record {
                            row.col(|ui| {
                                ui.label(fix_text(column));
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

    fn generate_certificates(&mut self) -> anyhow::Result<()> {
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
            let font_size = self.font_size;

            self.certificates_window_open = true;
            self.status = String::from("Creating...");

            self.t_handle = Some(std::thread::spawn(move || {
                records.par_iter().for_each(move |record| {
                    let filename = format!("{}-{}.png", &record[0], &record[1]);
                    generate_certificate(
                        record,
                        points.clone(),
                        template.clone(),
                        &filename,
                        font_size,
                    );
                });
            }));
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

    fn send_emails(&mut self) -> anyhow::Result<()> {
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
            let email_creds = self.config.email.clone();
            let Some(email_index) = self
                .columns
                .iter()
                .position(|s| s.to_lowercase() == "email" || s == "البريد الالكتروني") else {
                self.send_email_window_open = true;
                self.status = String::from("No email column");
                return Ok(());
            };
            let font_size = self.font_size;

            self.send_email_window_open = true;
            self.status = String::from("Sending...");
            self.t_handle = Some(std::thread::spawn(move || {
                records.par_iter().for_each(|record| {
                    let filename = format!("{}-{}.png", &record[0], &record[1]);
                    generate_certificate(
                        record,
                        points.clone(),
                        template.clone(),
                        &filename,
                        font_size,
                    );
                    send_email(email_creds.clone(), &filename, &record[email_index])
                        .expect("Send Email");
                });
            }));
        }

        Ok(())
    }
}

impl App for CertApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_fonts(add_fonts());
        egui::TopBottomPanel::bottom("BottomPanel").show(ctx, |ui| {
            ui.set_enabled(!self.template_window_open);
            ui.set_enabled(!self.email_window_open);
            ui.set_enabled(!self.certificates_window_open);
            ui.set_enabled(!self.send_email_window_open);
            ui.horizontal(|ui| {
                let button = ui.add_sized([20., 30.], Button::new("Import CSV"));
                if button.clicked() {
                    self.import_csv().expect("import csv");
                }
                let button = ui.add_sized([20., 30.], Button::new("Import Template"));
                if button.clicked() {
                    self.pick_template().expect("pick template");
                }
                let button = ui.add_sized([20., 30.], Button::new("Template Layout"));
                if button.clicked() {
                    self.template_window_open = true;
                }
                let button = ui.add_sized([20., 30.], Button::new("Create"));
                if button.clicked() {
                    self.generate_certificates().expect("certificates");
                }
                // let button = ui.add_sized([20., 30.], Button::new("Email Credentials"));
                // if button.clicked() {
                //     self.email_window_open = true;
                // }
                let button = ui.add_sized([20., 30.], Button::new("Send Email"));
                if button.clicked() {
                    println!("Send Email");
                    self.send_emails().expect("Send Emails");
                }
                ui.add(egui::Slider::new(&mut self.font_size, 0.0..=100.).text("Font size"))
            });
            ui.set_min_size(Vec2::new(ui.available_height(), 20.));
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.set_enabled(!self.email_window_open);
            ui.set_enabled(!self.send_email_window_open);
            ui.set_enabled(!self.certificates_window_open);
            ui.set_enabled(!self.template_window_open);
            self.table(ui);
        });

        egui::Window::new("Draw Areas")
            .open(&mut self.template_window_open)
            .anchor(Align2::CENTER_CENTER, [0., 0.])
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                if self.rects.len() < 1 {
                    ui.label("Add a CSV file");
                    return;
                }
                let Some(template) = &self.image else {
                    ui.label("Choose a template");
                    return;
                };
                let (current, current_color) = &mut self.rects[self.current_rect];

                ui.label(
                    RichText::new(format!(
                        "Column: {}",
                        fix_text(&self.columns[self.current_rect])
                    ))
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

                for rect in &self.rects {
                    ui.painter().rect(
                        Rect {
                            max: (rect.0.p1.max(rect.0.p2) + offset),
                            min: (rect.0.p1.min(rect.0.p2) + offset),
                        },
                        Rounding::none(),
                        Color32::TRANSPARENT,
                        Stroke::new(3., rect.1),
                    );
                }
                ui.horizontal(|ui| {
                    if ui.button("Clear").clicked() {
                        self.rects[self.current_rect].0 = TextRect::default();
                    }

                    for (i, column) in self.columns.iter().enumerate() {
                        if ui.button(fix_text(column)).clicked() {
                            self.current_rect = i;
                        }
                    }
                });
            });
        egui::Window::new("Email Credentials")
            .open(&mut self.email_window_open)
            .anchor(Align2::CENTER_CENTER, [0., 0.])
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label("Email");
                ui.text_edit_singleline(&mut self.current_email_creds.username);
                ui.label("Password");
                ui.add(
                    egui::TextEdit::singleline(&mut self.current_email_creds.password)
                        .password(true),
                );
                ui.horizontal(|ui| {
                    if ui.button("Save").clicked() {
                        let config_dir =
                            dirs::config_dir().expect("config directory").join("certs/");
                        self.config.email = self.current_email_creds.clone();
                        let current_config =
                            toml::to_string(&self.config).expect("Config to string");
                        fs::write(config_dir.join("config.toml"), current_config)
                            .expect("save config");
                    }
                    if ui.button("Clear").clicked() {
                        let config_dir =
                            dirs::config_dir().expect("config directory").join("certs/");
                        self.config.email = EmailCreds::default();
                        self.current_email_creds = EmailCreds::default();
                        let current_config =
                            toml::to_string(&self.config).expect("Config to string");
                        fs::write(config_dir.join("config.toml"), current_config)
                            .expect("save config");
                    }
                })
            });

        egui::Window::new("Create Certificates")
            .open(&mut self.certificates_window_open)
            .anchor(Align2::CENTER_CENTER, [0., 0.])
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label(self.status.clone());
            });

        egui::Window::new("Send Email")
            .open(&mut self.send_email_window_open)
            .anchor(Align2::CENTER_CENTER, [0., 0.])
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                if self.config.email.username.is_empty() || self.config.email.password.is_empty() {
                    ui.label("Add Email credentials");
                } else {
                    ui.label(self.status.clone());
                }
            });

        if let Some(t_handle) = &self.t_handle {
            if t_handle.is_finished() {
                self.status = String::from("Finished!");
                self.t_handle = None;
            }
        }
    }
}
