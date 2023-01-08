#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::fs;

use certs::{add_fonts, generate_certificate, Record};
use eframe::{
    egui::{self, Button, Ui},
    epaint::Vec2,
    App,
};
use egui_extras::{Column, TableBuilder};
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

#[derive(Default)]
struct CertApp {
    records: Vec<Record>,
}

impl CertApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self::default()
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
            let c = self.records.clone();

            std::thread::spawn(move || {
                c.par_iter().for_each(|record| {
                    generate_certificate(&record);
                });
            });
        }
        Ok(())
    }
}

impl App for CertApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.set_fonts(add_fonts());
        egui::TopBottomPanel::bottom("BottomPanel").show(ctx, |ui| {
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
            });
            ui.set_min_size(Vec2::new(ui.available_height(), 20.));
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            self.table(ui);
        });
    }
}