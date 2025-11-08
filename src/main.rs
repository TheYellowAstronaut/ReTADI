// Cargo.toml dependencies:
// [dependencies]
// eframe = "0.28"
// egui = "0.28"
// qrcode = "0.14"
// image = "0.25"
// tokio = { version = "1", features = ["full"] }
// axum = "0.7"
// tower = "0.4"
// tower-http = { version = "0.5", features = ["cors", "fs", "set-header"] }
// local-ip-address = "0.6"

use eframe::egui;
use qrcode::QrCode;
use image::Luma;
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;
use tower_http::services::ServeDir;

const ACCENT: egui::Color32 = egui::Color32::from_rgb(0, 255, 195);
const ALT_ACCENT: egui::Color32 = egui::Color32::from_rgb(0, 71, 54);
const BG_DARK: egui::Color32 = egui::Color32::from_rgb(34, 34, 34);
const BG_CARD: egui::Color32 = egui::Color32::from_rgb(51, 51, 51);

#[derive(Debug, Clone, Copy, PartialEq)]
enum Tab {
    Connect,
    Applets,
    Settings,
}

struct ServerState {
    is_running: bool,
    url: String,
    port: u16,
}

pub struct QrServerApp {
    server_state: Arc<Mutex<ServerState>>,
    qr_texture: Option<egui::TextureHandle>,
    runtime: Runtime,
    current_tab: Tab,
}

impl Default for QrServerApp {
    fn default() -> Self {
        Self {
            server_state: Arc::new(Mutex::new(ServerState {
                is_running: false,
                url: String::new(),
                port: 3000,
            })),
            qr_texture: None,
            runtime: Runtime::new().unwrap(),
            current_tab: Tab::Connect,
        }
    }
}

impl QrServerApp {
    fn generate_qr_texture(&mut self, ctx: &egui::Context, url: &str) {
        if let Ok(code) = QrCode::new(url.as_bytes()) {
            let image = code.render::<Luma<u8>>()
                .min_dimensions(400, 400)
                .max_dimensions(400, 400)
                .build();

            let size = [image.width() as usize, image.height() as usize];
            let pixels: Vec<egui::Color32> = image.pixels()
                .map(|p| {
                    if p.0[0] == 0 {
                        egui::Color32::BLACK
                    } else {
                        egui::Color32::WHITE
                    }
                })
                .collect();

            let color_image = egui::ColorImage {
                size,
                pixels,
            };

            self.qr_texture = Some(ctx.load_texture(
                "qr_code",
                color_image,
                egui::TextureOptions::NEAREST,
            ));
        }
    }

    fn start_server(&mut self) {
        let state = self.server_state.clone();

        self.runtime.spawn(async move {
            let serve_dir = ServeDir::new("clientside")
                .append_index_html_on_directories(true);

            let app = axum::Router::new()
                .route("/api/connect", axum::routing::post(handle_connect))
                .nest_service("/", serve_dir)
                .layer(
                    tower_http::cors::CorsLayer::permissive()
                )
                .layer(
                    tower::ServiceBuilder::new()
                        .layer(tower_http::set_header::SetResponseHeaderLayer::overriding(
                            axum::http::header::CACHE_CONTROL,
                            axum::http::HeaderValue::from_static("no-cache, no-store, must-revalidate"),
                        ))
                );

            let port = {
                let s = state.lock().unwrap();
                s.port
            };

            let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
                .await
                .unwrap();

            let local_ip = local_ip_address::local_ip().unwrap_or_else(|_| "127.0.0.1".parse().unwrap());
            let url = format!("http://{}:{}", local_ip, port);

            {
                let mut s = state.lock().unwrap();
                s.is_running = true;
                s.url = url;
            }

            axum::serve(listener, app).await.unwrap();
        });
    }

    fn render_tab_bar(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(10.0);
            ui.heading(egui::RichText::new("ReTADI")
                .size(22.0)
                .color(ACCENT));
            ui.add_space(30.0);
            let tab_button = |ui: &mut egui::Ui, label: &str, tab: Tab, current: Tab| {
                let is_selected = tab == current;
                let bg_color = if is_selected { ALT_ACCENT } else { BG_CARD };
                let hover_color = if is_selected {
                    egui::Color32::from_rgb(0, 133, 101)
                } else {
                    egui::Color32::from_rgb(70, 70, 70)
                };
                let (rect, response) = ui.allocate_exact_size(
                    egui::vec2(140.0, 45.0),
                    egui::Sense::click(),
                );
                let visuals = ui.style().interact(&response);
                let fill = if response.hovered() { hover_color } else { bg_color };
                let text_color = if response.hovered() {
                    egui::Color32::WHITE
                } else {
                    egui::Color32::LIGHT_GRAY
                };
                ui.painter().rect(
                    rect,
                    10.0,
                    fill,
                    visuals.bg_stroke,
                );
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    label,
                    egui::FontId::proportional(16.0),
                    text_color,
                );
                if response.clicked() {
                    return true;
                }
                false
            };
            if tab_button(ui, "Connect", Tab::Connect, self.current_tab) {
                self.current_tab = Tab::Connect;
            }
            ui.add_space(10.0);
            if tab_button(ui, "Applets", Tab::Applets, self.current_tab) {
                self.current_tab = Tab::Applets;
            }
            ui.add_space(10.0);
            if tab_button(ui, "Settings", Tab::Settings, self.current_tab) {
                self.current_tab = Tab::Settings;
            }
            ui.add_space(40.0);
        });
    }

    fn render_connect_tab(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);

            ui.heading(egui::RichText::new("Device Connection")
                .size(32.0)
                .color(ACCENT));

            ui.add_space(30.0);

            let is_running = {
                let state = self.server_state.lock().unwrap();
                state.is_running
            };

            if !is_running {
                ui.add_space(20.0);

                let start_btn = egui::Button::new(
                    egui::RichText::new("Start Server")
                        .size(20.0)
                        .color(BG_DARK)
                )
                .fill(ACCENT)
                .min_size(egui::vec2(200.0, 50.0))
                .rounding(10.0);

                if ui.add(start_btn).clicked() {
                    self.start_server();
                }
            } else {
                let url = {
                    let state = self.server_state.lock().unwrap();
                    state.url.clone()
                };

                if self.qr_texture.is_none() && !url.is_empty() {
                    self.generate_qr_texture(ctx, &url);
                }

                egui::Frame::none()
                    .fill(BG_CARD)
                    .rounding(15.0)
                    .inner_margin(30.0)
                    .show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.label(egui::RichText::new("Server Running")
                                .size(24.0)
                                .color(ACCENT));

                            ui.add_space(10.0);

                            ui.label(egui::RichText::new(&url)
                                .size(16.0)
                                .color(egui::Color32::WHITE));
                            
                            if let Some(texture) = &self.qr_texture {
                                ui.add_space(20.0);
                                ui.label(egui::RichText::new("Scan to Connect")
                                    .size(18.0)
                                    .color(egui::Color32::LIGHT_GRAY));
                                ui.add_space(10.0);
        
                                let img_size = egui::vec2(300.0, 300.0);
                                ui.image((texture.id(), img_size));
                            }
                            ui.add_space(20.0);
                        });
                    });
            }
        });
    }

    fn render_applets_tab(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(30.0);

            ui.heading(egui::RichText::new("Applets")
                .size(32.0)
                .color(ACCENT));

            ui.add_space(20.0);

            ui.label(egui::RichText::new("Manage and install applets for your device")
                .size(14.0)
                .color(egui::Color32::LIGHT_GRAY));

            ui.add_space(30.0);
        });

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.vertical(|ui| {
                ui.set_max_width(550.0);

                for i in 1..=5 {
                    egui::Frame::none()
                        .fill(BG_CARD)
                        .rounding(12.0)
                        .inner_margin(20.0)
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(egui::RichText::new(format!("Applet {}", i))
                                        .size(18.0)
                                        .color(egui::Color32::WHITE));

                                    ui.add_space(5.0);

                                    ui.label(egui::RichText::new("Description of the applet functionality")
                                        .size(13.0)
                                        .color(egui::Color32::GRAY));
                                });

                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    let install_btn = egui::Button::new(
                                        egui::RichText::new("Install")
                                            .size(14.0)
                                            .color(BG_DARK)
                                    )
                                    .fill(ACCENT)
                                    .rounding(8.0);

                                    ui.add(install_btn);
                                });
                            });
                        });

                    ui.add_space(12.0);
                }
            });
        });
    }

    fn render_settings_tab(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(30.0);

            ui.heading(egui::RichText::new("Settings")
                .size(32.0)
                .color(ACCENT));

            ui.add_space(20.0);
        });

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.vertical(|ui| {
                ui.set_max_width(500.0);

                // Server Settings
                egui::Frame::none()
                    .fill(BG_CARD)
                    .rounding(12.0)
                    .inner_margin(20.0)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("Server Settings")
                            .size(20.0)
                            .color(ACCENT));

                        ui.add_space(15.0);

                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Port:")
                                .size(14.0)
                                .color(egui::Color32::WHITE));
                            ui.add_space(10.0);
                            ui.label("3000");
                        });

                        ui.add_space(10.0);

                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Auto-start:")
                                .size(14.0)
                                .color(egui::Color32::WHITE));
                            ui.add_space(10.0);
                            ui.checkbox(&mut false, "");
                        });
                    });

                ui.add_space(15.0);

                // About Section
                egui::Frame::none()
                    .fill(BG_CARD)
                    .rounding(12.0)
                    .inner_margin(20.0)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("About")
                            .size(20.0)
                            .color(ACCENT));

                        ui.add_space(15.0);

                        ui.label(egui::RichText::new("ReTADI Server v0.1.0")
                            .size(14.0)
                            .color(egui::Color32::LIGHT_GRAY));

                        ui.add_space(5.0);

                        ui.label(egui::RichText::new("Remote Tablet Display Interface")
                            .size(12.0)
                            .color(egui::Color32::GRAY));
                    });
            });
        });
    }
}

async fn handle_connect(body: String) -> &'static str {
    println!("Device connected: {}", body);
    "Connected successfully"
}

impl eframe::App for QrServerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut style = (*ctx.style()).clone();
        style.visuals.window_fill = BG_DARK;
        style.visuals.panel_fill = BG_DARK;
        style.visuals.extreme_bg_color = BG_CARD;
        style.spacing.button_padding = egui::vec2(20.0, 14.0);
        style.spacing.item_spacing = egui::vec2(12.0, 12.0);
        ctx.set_style(style);
        // Side panel for tabs
        egui::SidePanel::left("tab_bar")
            .resizable(false)
            .exact_width(170.0)
            .frame(egui::Frame::none().fill(BG_CARD).inner_margin(egui::Margin::same(10.0)))
            .show(ctx, |ui| {
                self.render_tab_bar(ui);
            });
        // Main content area
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(BG_DARK).inner_margin(egui::Margin::same(20.0)))
            .show(ctx, |ui| {
                match self.current_tab {
                    Tab::Connect => self.render_connect_tab(ctx, ui),
                    Tab::Applets => self.render_applets_tab(ui),
                    Tab::Settings => self.render_settings_tab(ui),
                }
            });
    }
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([756.0, 699.3])
            .with_min_inner_size([756.0, 699.3]),
        ..Default::default()
    };

    eframe::run_native(
        "ReTADI Server",
        options,
        Box::new(|_cc| Ok(Box::<QrServerApp>::default())),
    )
}
