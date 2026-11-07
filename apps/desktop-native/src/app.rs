use std::time::Instant;

use eframe::egui;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

use crate::backend::{Backend, Cmd, Evt, DEFAULT_SESSION};
use crate::config_io::{self, ConfigForm};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Chat,
    Status,
    Config,
    Secrets,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionItem {
    pub id: String,
    pub title: String,
}

#[derive(Clone)]
struct Msg {
    is_user: bool,
    content: String,
    /// Footer for an assistant reply, e.g. token usage + elapsed time.
    meta: Option<String>,
}

// Zoom factors for the 3 font-size levels (scales the whole UI).
const FONT_ZOOM: [f32; 3] = [1.0, 1.25, 1.5];
const FONT_LABELS: [&str; 3] = ["S", "M", "L"];

// First-run example prompts shown on the empty chat screen.
const EXAMPLES: [(&str, &str); 4] = [
    (
        "Summarize Git Log",
        "Summarize the latest git commits of this repository and explain key changes.",
    ),
    (
        "Search AI Agent News",
        "Web search: latest developments and breakthroughs in AI agents this week.",
    ),
    (
        "Analyze README.md",
        "Read README.md and summarize the architecture, structure, and quickstart.",
    ),
    (
        "Create Backup Script",
        "Write a robust bash script to archive and compress a directory into tar.gz.",
    ),
];

pub struct App {
    page: Page,
    backend: Backend,
    boot_ms: u128,
    md_cache: CommonMarkCache,

    // appearance
    dark: bool,
    font_level: usize,

    // sessions
    sessions: Vec<SessionItem>,
    active_session_id: String,

    // chat
    input: String,
    messages: Vec<Msg>,
    streaming: bool,
    /// When the current request was sent — drives the elapsed-time readout.
    send_at: Option<Instant>,
    /// Name of the tool the agent is currently running, if any.
    active_tool: Option<String>,
    hint: String,
    routing: Vec<(String, String)>,
    default_model: String,
    ingest_input: String,
    show_ingest_bar: bool,

    // config
    config: ConfigForm,
    config_status: Option<String>,

    // secrets
    secrets: Vec<config_io::EnvEntry>,
    env_keys: std::collections::HashSet<String>,
    new_key: String,
    new_value: String,
    secrets_status: Option<String>,
    screenshot_path: Option<String>,
    frame_count: u32,
}

impl App {
    pub fn new(
        backend: Backend,
        boot_ms: u128,
        dark: bool,
        font_level: usize,
        saved_sessions: Option<Vec<SessionItem>>,
        saved_active_session: Option<String>,
        screenshot_path: Option<String>,
    ) -> Self {
        let config = ConfigForm::load();
        let routing = config.routing.clone();
        let default_model = config.model.clone();
        let secrets = config_io::list_env();
        let env_keys = secrets.iter().map(|e| e.key.clone()).collect();

        let sessions = saved_sessions.filter(|s| !s.is_empty()).unwrap_or_else(|| {
            vec![SessionItem {
                id: DEFAULT_SESSION.to_string(),
                title: "Default Chat".to_string(),
            }]
        });

        let active_session_id = saved_active_session
            .filter(|id| sessions.iter().any(|s| s.id == *id))
            .unwrap_or_else(|| sessions[0].id.clone());

        // If active session is different from default, trigger history load
        if active_session_id != DEFAULT_SESSION {
            let _ = backend
                .cmd_tx
                .send(Cmd::LoadHistory(active_session_id.clone()));
        }

        Self {
            page: Page::Chat,
            backend,
            boot_ms,
            md_cache: CommonMarkCache::default(),
            dark,
            font_level: font_level.min(2),
            sessions,
            active_session_id,
            input: String::new(),
            messages: Vec::new(),
            streaming: false,
            send_at: None,
            active_tool: None,
            hint: String::new(),
            routing,
            default_model,
            ingest_input: String::new(),
            show_ingest_bar: false,
            config,
            config_status: None,
            secrets,
            env_keys,
            new_key: String::new(),
            new_value: String::new(),
            secrets_status: None,
            screenshot_path,
            frame_count: 0,
        }
    }

    fn refresh_secrets(&mut self) {
        self.secrets = config_io::list_env();
        self.env_keys = self.secrets.iter().map(|e| e.key.clone()).collect();
    }

    fn send(&mut self) {
        let text = self.input.trim().to_string();
        if text.is_empty() || self.streaming {
            return;
        }
        self.input.clear();

        // Update session title if default
        if let Some(sess) = self
            .sessions
            .iter_mut()
            .find(|s| s.id == self.active_session_id)
        {
            if sess.title == "New Chat" || sess.title == "Default Chat" {
                let title: String = text.chars().take(22).collect();
                sess.title = if text.chars().count() > 22 {
                    format!("{title}…")
                } else {
                    title
                };
            }
        }

        self.messages.push(Msg {
            is_user: true,
            content: text.clone(),
            meta: None,
        });
        self.messages.push(Msg {
            is_user: false,
            content: String::new(),
            meta: None,
        });
        self.streaming = true;
        self.send_at = Some(Instant::now());
        self.active_tool = None;
        let hint = if self.hint.is_empty() {
            None
        } else {
            Some(self.hint.clone())
        };
        let _ = self.backend.cmd_tx.send(Cmd::Chat {
            session_key: self.active_session_id.clone(),
            text,
            hint,
        });
    }
}

fn custom_visuals(dark: bool) -> egui::Visuals {
    if dark {
        let mut v = egui::Visuals::dark();
        v.override_text_color = Some(egui::Color32::from_rgb(235, 238, 245));
        v.panel_fill = egui::Color32::from_rgb(18, 19, 25);
        v.window_fill = egui::Color32::from_rgb(22, 24, 32);
        v.extreme_bg_color = egui::Color32::from_rgb(14, 15, 20);

        v.widgets.inactive.bg_fill = egui::Color32::from_rgb(28, 30, 40);
        v.widgets.inactive.bg_stroke =
            egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(44, 48, 62));
        v.widgets.inactive.rounding = egui::Rounding::same(8.0);
        v.widgets.inactive.fg_stroke =
            egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(205, 210, 225));

        v.widgets.hovered.bg_fill = egui::Color32::from_rgb(38, 42, 56);
        v.widgets.hovered.bg_stroke =
            egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(70, 76, 98));
        v.widgets.hovered.rounding = egui::Rounding::same(8.0);
        v.widgets.hovered.fg_stroke = egui::Stroke::new(1.0_f32, egui::Color32::WHITE);

        v.widgets.active.bg_fill = egui::Color32::from_rgb(225, 165, 40);
        v.widgets.active.bg_stroke = egui::Stroke::NONE;
        v.widgets.active.rounding = egui::Rounding::same(8.0);
        v.widgets.active.fg_stroke = egui::Stroke::new(1.0_f32, egui::Color32::BLACK);

        v.widgets.open.bg_fill = egui::Color32::from_rgb(30, 33, 44);
        v.widgets.open.rounding = egui::Rounding::same(8.0);

        v.selection.bg_fill = egui::Color32::from_rgb(225, 165, 40).linear_multiply(0.35);
        v.selection.stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(230, 175, 50));

        v.window_rounding = egui::Rounding::same(12.0);
        v.menu_rounding = egui::Rounding::same(8.0);
        v
    } else {
        let mut v = egui::Visuals::light();
        v.panel_fill = egui::Color32::from_rgb(247, 248, 252);
        v.window_fill = egui::Color32::from_rgb(255, 255, 255);
        v.extreme_bg_color = egui::Color32::from_rgb(240, 242, 248);

        v.widgets.inactive.rounding = egui::Rounding::same(8.0);
        v.widgets.hovered.rounding = egui::Rounding::same(8.0);
        v.widgets.active.rounding = egui::Rounding::same(8.0);
        v.window_rounding = egui::Rounding::same(12.0);
        v
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply refined modern visuals
        ctx.set_visuals(custom_visuals(self.dark));
        ctx.set_zoom_factor(FONT_ZOOM[self.font_level]);

        // Automated screenshot capture mode
        if let Some(path) = &self.screenshot_path {
            self.frame_count += 1;
            if self.frame_count == 1 {
                self.dark = true;
                self.font_level = 0;
                self.sessions = vec![
                    SessionItem {
                        id: "desktop:rag".to_string(),
                        title: "RAG Docs & Summary".to_string(),
                    },
                    SessionItem {
                        id: "desktop:default".to_string(),
                        title: "Default Chat".to_string(),
                    },
                ];
                self.active_session_id = "desktop:rag".to_string();
                self.messages.clear();
                self.messages.push(Msg {
                    is_user: true,
                    content:
                        "Index README.md for local RAG and summarize the core agent architecture."
                            .to_string(),
                    meta: None,
                });
                self.messages.push(Msg {
                    is_user: false,
                    content: "**Indexed Document:** `README.md` (6 chunks)

Added to local RAG memory. Ready to answer questions from this knowledge base!"
                        .to_string(),
                    meta: None,
                });
                self.messages.push(Msg {
                    is_user: false,
                    content: "### Garudust v0.13.9 — Modern AI Agent

System is ready with **Semantic Hybrid RAG** (Vector + FTS5 Full-Text Search):

1. **Multi-Session Chat**: Create, switch, and manage independent conversations in the sidebar.
2. **Drag & Drop Document Ingestion**: Drop files anywhere onto the window to instantly index them.
3. **Modern Minimalist UI**: Sleek Slate Dark aesthetic with an ergonomic floating prompt bar.

Ask any question to explore the indexed documentation!"
                        .to_string(),
                    meta: Some("320 tokens (95 in · 225 out) · 1.1s".to_string()),
                });
            }

            if self.frame_count == 3 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
            }

            for event in &ctx.input(|i| i.raw.events.clone()) {
                if let egui::Event::Screenshot { image, .. } = event {
                    let w = image.size[0] as u32;
                    let h = image.size[1] as u32;
                    if let Some(buf) = image::RgbaImage::from_raw(w, h, image.as_raw().to_vec()) {
                        let _ = buf.save(path);
                    }
                    std::process::exit(0);
                }
            }
            ctx.request_repaint();
        }

        // Drain backend events.
        while let Ok(evt) = self.backend.evt_rx.try_recv() {
            match evt {
                Evt::History(pairs) => {
                    self.messages.clear();
                    for (user, assistant) in pairs {
                        self.messages.push(Msg {
                            is_user: true,
                            content: user,
                            meta: None,
                        });
                        self.messages.push(Msg {
                            is_user: false,
                            content: assistant,
                            meta: None,
                        });
                    }
                }
                Evt::Delta(d) => {
                    // First token of the answer means the tool round is over.
                    self.active_tool = None;
                    if let Some(last) = self.messages.last_mut() {
                        last.content.push_str(&d);
                    }
                }
                Evt::Tool(name) => self.active_tool = Some(name),
                Evt::Done(usage) => {
                    if let Some(u) = usage {
                        let secs = self.send_at.map_or(0.0, |t| t.elapsed().as_secs_f32());
                        let total = u.input + u.output;
                        if let Some(last) = self.messages.last_mut() {
                            if !last.is_user {
                                last.meta = Some(format!(
                                    "{total} tokens ({} in · {} out) · {secs:.1}s",
                                    u.input, u.output
                                ));
                            }
                        }
                    }
                    self.streaming = false;
                    self.send_at = None;
                    self.active_tool = None;
                }
                Evt::FileIngested { file_name, chunks } => {
                    self.messages.push(Msg {
                        is_user: false,
                        content: format!(
                            "📄 **Indexed Document:** `{file_name}` ({chunks} chunks)\n\nAdded to local RAG memory. Ready to answer questions from this knowledge base!"
                        ),
                        meta: None,
                    });
                }
                Evt::Error(e) => {
                    if let Some(last) = self.messages.last_mut() {
                        if last.content.is_empty() {
                            last.content = format!("⚠ {e}");
                        }
                    }
                    self.streaming = false;
                    self.send_at = None;
                    self.active_tool = None;
                }
            }
        }

        // Left Navigation Sidebar (220px)
        egui::SidePanel::left("nav")
            .exact_width(220.0)
            .resizable(false)
            .show(ctx, |ui| {
                ui.add_space(10.0);

                // Branding header: full banner logo
                ui.add(
                    egui::Image::from_bytes(
                        "bytes://garudust-logo",
                        include_bytes!("../../../assets/logo-agent.jpg").as_slice(),
                    )
                    .max_width(ui.available_width())
                    .rounding(6.0),
                );
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("GARUDUST")
                            .strong()
                            .size(13.0)
                            .color(egui::Color32::from_rgb(225, 165, 40)),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "v{} · {}ms",
                                env!("CARGO_PKG_VERSION"),
                                self.boot_ms
                            ))
                            .small()
                            .weak(),
                        );
                    });
                });

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(6.0);

                // Primary Navigation tabs with modern button styling
                for (p, label) in [
                    (Page::Chat, "Chat"),
                    (Page::Status, "Status"),
                    (Page::Config, "Config"),
                    (Page::Secrets, "Secrets"),
                ] {
                    let selected = self.page == p;
                    let btn_color = if selected {
                        egui::Color32::WHITE
                    } else {
                        egui::Color32::from_rgb(175, 180, 195)
                    };
                    let bg_color = if selected {
                        egui::Color32::from_rgb(38, 42, 56)
                    } else {
                        egui::Color32::TRANSPARENT
                    };
                    let stroke = if selected {
                        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(225, 165, 40))
                    } else {
                        egui::Stroke::NONE
                    };

                    let resp = ui.add(
                        egui::Button::new(egui::RichText::new(label).size(13.0).color(btn_color))
                            .fill(bg_color)
                            .stroke(stroke)
                            .rounding(8.0)
                            .min_size(egui::vec2(ui.available_width(), 32.0)),
                    );

                    if resp.clicked() {
                        self.page = p;
                        if p == Page::Secrets {
                            self.refresh_secrets();
                        }
                    }
                    ui.add_space(2.0);
                }

                // If on Chat page, show Sessions Sidebar
                if self.page == Page::Chat {
                    ui.add_space(12.0);
                    ui.separator();
                    ui.add_space(6.0);

                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("CHATS")
                                .strong()
                                .size(11.0)
                                .color(egui::Color32::from_rgb(140, 145, 165)),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let new_btn = ui.add(
                                egui::Button::new(
                                    egui::RichText::new("+ New")
                                        .size(11.5)
                                        .color(egui::Color32::from_rgb(225, 165, 40))
                                        .strong(),
                                )
                                .fill(egui::Color32::from_rgb(34, 38, 50))
                                .stroke(egui::Stroke::new(
                                    1.0_f32,
                                    egui::Color32::from_rgb(55, 60, 80),
                                ))
                                .rounding(6.0),
                            );

                            if new_btn.on_hover_text("Create new chat").clicked() && !self.streaming
                            {
                                let rand_id: String =
                                    uuid::Uuid::new_v4().to_string().chars().take(8).collect();
                                let new_id = format!("desktop:{rand_id}");
                                self.sessions.insert(
                                    0,
                                    SessionItem {
                                        id: new_id.clone(),
                                        title: "New Chat".to_string(),
                                    },
                                );
                                self.active_session_id = new_id.clone();
                                self.messages.clear();
                                self.streaming = false;
                                self.send_at = None;
                                self.active_tool = None;
                                let _ = self.backend.cmd_tx.send(Cmd::ClearSession(new_id));
                            }
                        });
                    });

                    ui.add_space(6.0);

                    let mut switch_to: Option<String> = None;
                    let mut delete_sess: Option<usize> = None;

                    egui::ScrollArea::vertical()
                        .max_height(280.0)
                        .show(ui, |ui| {
                            for (i, s) in self.sessions.iter().enumerate() {
                                let is_active = self.active_session_id == s.id;
                                let title = if s.title.is_empty() {
                                    "New Chat"
                                } else {
                                    &s.title
                                };

                                let bg = if is_active {
                                    egui::Color32::from_rgb(34, 38, 50)
                                } else {
                                    egui::Color32::TRANSPARENT
                                };
                                let stroke = if is_active {
                                    egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(58, 64, 86))
                                } else {
                                    egui::Stroke::NONE
                                };

                                egui::Frame::none()
                                    .fill(bg)
                                    .stroke(stroke)
                                    .rounding(6.0)
                                    .inner_margin(egui::Margin::symmetric(6.0, 5.0))
                                    .show(ui, |ui| {
                                        ui.horizontal(|ui| {
                                            let indicator = if is_active { "•" } else { " " };
                                            let ind_color = if is_active {
                                                egui::Color32::from_rgb(225, 165, 40)
                                            } else {
                                                egui::Color32::from_rgb(100, 105, 120)
                                            };
                                            ui.label(
                                                egui::RichText::new(indicator)
                                                    .color(ind_color)
                                                    .size(9.0),
                                            );

                                            let label_color = if is_active {
                                                egui::Color32::WHITE
                                            } else {
                                                egui::Color32::from_rgb(175, 180, 195)
                                            };

                                            let display_title: String =
                                                title.chars().take(18).collect();
                                            let display_title = if title.chars().count() > 18 {
                                                format!("{display_title}…")
                                            } else {
                                                display_title
                                            };

                                            let text_resp = ui.add(
                                                egui::Label::new(
                                                    egui::RichText::new(display_title)
                                                        .color(label_color)
                                                        .size(12.5),
                                                )
                                                .sense(egui::Sense::click()),
                                            );
                                            if text_resp.clicked() && !is_active && !self.streaming
                                            {
                                                switch_to = Some(s.id.clone());
                                            }

                                            if self.sessions.len() > 1 {
                                                ui.with_layout(
                                                    egui::Layout::right_to_left(
                                                        egui::Align::Center,
                                                    ),
                                                    |ui| {
                                                        if ui
                                                            .add(
                                                                egui::Button::new(
                                                                    egui::RichText::new("×")
                                                                        .size(10.0),
                                                                )
                                                                .frame(false),
                                                            )
                                                            .on_hover_text("Delete this session")
                                                            .clicked()
                                                            && !self.streaming
                                                        {
                                                            delete_sess = Some(i);
                                                        }
                                                    },
                                                );
                                            }
                                        });
                                    });
                                ui.add_space(2.0);
                            }
                        });

                    if let Some(target) = switch_to {
                        self.active_session_id = target.clone();
                        self.messages.clear();
                        self.streaming = false;
                        self.send_at = None;
                        self.active_tool = None;
                        let _ = self.backend.cmd_tx.send(Cmd::LoadHistory(target));
                    }

                    if let Some(del_idx) = delete_sess {
                        let del_id = self.sessions[del_idx].id.clone();
                        self.sessions.remove(del_idx);
                        if self.active_session_id == del_id {
                            if let Some(first) = self.sessions.first() {
                                self.active_session_id = first.id.clone();
                                self.messages.clear();
                                let _ =
                                    self.backend.cmd_tx.send(Cmd::LoadHistory(first.id.clone()));
                            }
                        }
                        let _ = self.backend.cmd_tx.send(Cmd::ClearSession(del_id));
                    }
                }

                // Appearance controls pinned to the bottom of sidebar
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                    ui.add_space(10.0);

                    // Font Zoom selector
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Font Size:").small().weak());
                        for (i, label) in FONT_LABELS.iter().enumerate() {
                            ui.selectable_value(&mut self.font_level, i, *label);
                        }
                    });

                    ui.add_space(4.0);

                    // Theme toggle
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Theme:").small().weak());
                        ui.selectable_value(&mut self.dark, true, "Dark");
                        ui.selectable_value(&mut self.dark, false, "Light");
                    });

                    ui.add_space(4.0);
                    ui.separator();
                });
            });

        match self.page {
            Page::Chat => self.ui_chat(ctx),
            Page::Status => self.ui_status(ctx),
            Page::Config => self.ui_config(ctx),
            Page::Secrets => self.ui_secrets(ctx),
        }

        if self.streaming {
            ctx.request_repaint();
        }
    }

    // Persist appearance prefs and sessions
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "dark", &self.dark);
        eframe::set_value(storage, "font_level", &self.font_level);
        eframe::set_value(storage, "sessions", &self.sessions);
        eframe::set_value(storage, "active_session_id", &self.active_session_id);
    }
}

impl App {
    fn ui_chat(&mut self, ctx: &egui::Context) {
        // Drag-and-drop file ingestion support
        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        for file in dropped {
            if let Some(path_buf) = file.path {
                let path = path_buf.to_string_lossy().to_string();
                let _ = self.backend.cmd_tx.send(Cmd::IngestFile {
                    session_key: self.active_session_id.clone(),
                    path,
                });
            }
        }

        // Top Header in Chat: active title + Model pill + quick actions
        egui::TopBottomPanel::top("chat_header")
            .exact_height(42.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.add_space(4.0);
                    let active_title = self
                        .sessions
                        .iter()
                        .find(|s| s.id == self.active_session_id)
                        .map(|s| s.title.as_str())
                        .unwrap_or("Chat");

                    ui.label(egui::RichText::new(active_title).strong().size(13.5));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Reload button
                        if ui
                            .add(
                                egui::Button::new(egui::RichText::new("Reload").size(12.0))
                                    .rounding(6.0),
                            )
                            .on_hover_text("Reload Agent & Config")
                            .clicked()
                        {
                            let _ = self.backend.cmd_tx.send(Cmd::Reload);
                        }

                        // Clear Chat button
                        if ui
                            .add(
                                egui::Button::new(egui::RichText::new("Clear").size(12.0))
                                    .rounding(6.0),
                            )
                            .on_hover_text("Clear messages in this session")
                            .clicked()
                            && !self.streaming
                        {
                            self.messages.clear();
                            let _ = self
                                .backend
                                .cmd_tx
                                .send(Cmd::ClearSession(self.active_session_id.clone()));
                        }

                        // Model picker combobox
                        let sel = if self.hint.is_empty() {
                            format!("Default · {}", self.default_model)
                        } else {
                            format!("Model: {}", self.hint)
                        };

                        egui::ComboBox::from_id_salt("model_picker")
                            .selected_text(egui::RichText::new(sel).size(12.0))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.hint,
                                    String::new(),
                                    format!("Default · {}", self.default_model),
                                );
                                for (name, target) in &self.routing {
                                    ui.selectable_value(
                                        &mut self.hint,
                                        name.clone(),
                                        format!("{name} · {target}"),
                                    );
                                }
                            });
                    });
                });
            });

        // Bottom Panel: Floating Prompt Card
        egui::TopBottomPanel::bottom("input_panel")
            .frame(
                egui::Frame::none()
                    .fill(if self.dark {
                        egui::Color32::from_rgb(18, 19, 25)
                    } else {
                        egui::Color32::from_rgb(247, 248, 252)
                    })
                    .inner_margin(egui::Margin::symmetric(14.0, 10.0)),
            )
            .show(ctx, |ui| {
                // Ingest drawer if opened
                if self.show_ingest_bar {
                    egui::Frame::none()
                        .fill(if self.dark {
                            egui::Color32::from_rgb(26, 29, 39)
                        } else {
                            egui::Color32::from_rgb(238, 241, 248)
                        })
                        .stroke(egui::Stroke::new(
                            1.0_f32,
                            egui::Color32::from_rgb(52, 58, 76),
                        ))
                        .rounding(8.0)
                        .inner_margin(egui::Margin::symmetric(10.0, 6.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("RAG Ingest:").strong().size(12.0));
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.ingest_input)
                                        .hint_text("Absolute file path (e.g. /path/to/doc.pdf) or drop file here")
                                        .desired_width(ui.available_width() - 85.0),
                                );
                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new("Index")
                                                .color(egui::Color32::BLACK)
                                                .strong(),
                                        )
                                        .fill(egui::Color32::from_rgb(225, 165, 40))
                                        .rounding(6.0),
                                    )
                                    .clicked()
                                    && !self.ingest_input.trim().is_empty()
                                {
                                    let path = self.ingest_input.trim().to_string();
                                    self.ingest_input.clear();
                                    self.show_ingest_bar = false;
                                    let _ = self.backend.cmd_tx.send(Cmd::IngestFile {
                                        session_key: self.active_session_id.clone(),
                                        path,
                                    });
                                }
                            });
                        });
                    ui.add_space(6.0);
                }

                // Outer Prompt Card
                egui::Frame::none()
                    .fill(if self.dark {
                        egui::Color32::from_rgb(24, 26, 35)
                    } else {
                        egui::Color32::WHITE
                    })
                    .stroke(egui::Stroke::new(
                        1.0_f32,
                        if self.dark {
                            egui::Color32::from_rgb(46, 50, 66)
                        } else {
                            egui::Color32::from_rgb(215, 220, 230)
                        },
                    ))
                    .rounding(12.0)
                    .inner_margin(egui::Margin::symmetric(12.0, 8.0))
                    .show(ui, |ui| {
                        // Input text area
                        let resp = ui.add_enabled(
                            !self.streaming,
                            egui::TextEdit::multiline(&mut self.input)
                                .hint_text("Type a message to Garudust… (Enter to send · Shift+Enter for new line · Drop files for RAG)")
                                .desired_rows(2)
                                .desired_width(ui.available_width())
                                .frame(false),
                        );

                        ui.add_space(4.0);

                        // Actions toolbar inside the card
                        ui.horizontal(|ui| {
                            // Ingest toggle button
                            let ingest_label = if self.show_ingest_bar {
                                "Close Ingest"
                            } else {
                                "Attach (RAG)"
                            };
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new(ingest_label).size(11.5),
                                    )
                                    .frame(false),
                                )
                                .on_hover_text("Index document into RAG memory or drop files here")
                                .clicked()
                            {
                                self.show_ingest_bar = !self.show_ingest_bar;
                            }

                            // Send / Stop button aligned right
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let enter = resp.has_focus()
                                        && ui.input(|i| {
                                            i.key_pressed(egui::Key::Enter) && !i.modifiers.shift
                                        });

                                    if self.streaming {
                                        if ui
                                            .add(
                                                egui::Button::new(
                                                    egui::RichText::new("Stop")
                                                        .color(egui::Color32::WHITE)
                                                        .strong(),
                                                )
                                                .fill(egui::Color32::from_rgb(220, 53, 69))
                                                .rounding(8.0)
                                                .min_size(egui::vec2(70.0, 28.0)),
                                            )
                                            .clicked()
                                        {
                                            let _ = self.backend.cmd_tx.send(Cmd::Stop);
                                        }
                                    } else {
                                        let send_btn = ui.add(
                                            egui::Button::new(
                                                egui::RichText::new("Send")
                                                    .color(egui::Color32::BLACK)
                                                    .strong(),
                                            )
                                            .fill(egui::Color32::from_rgb(225, 165, 40))
                                            .rounding(8.0)
                                            .min_size(egui::vec2(64.0, 28.0)),
                                        );

                                        if send_btn.clicked() || enter {
                                            self.send();
                                            resp.request_focus();
                                        }
                                    }
                                },
                            );
                        });
                    });
            });

        // Chat Central Area
        let mut chosen: Option<String> = None;
        egui::CentralPanel::default().show(ctx, |ui| {
            // Hovered files visual indicator overlay
            let is_hovering_file = ctx.input(|i| !i.raw.hovered_files.is_empty());
            if is_hovering_file {
                egui::Frame::none()
                    .fill(egui::Color32::from_rgba_premultiplied(
                        225, 165, 40, 25,
                    ))
                    .stroke(egui::Stroke::new(
                        2.0_f32,
                        egui::Color32::from_rgb(225, 165, 40),
                    ))
                    .rounding(12.0)
                    .inner_margin(egui::Margin::symmetric(16.0, 12.0))
                    .show(ui, |ui| {
                        ui.horizontal_centered(|ui| {
                            ui.label(
                                egui::RichText::new("Drop files here to instantly index into RAG memory!")
                                    .size(14.0)
                                    .color(egui::Color32::from_rgb(225, 165, 40))
                                    .strong(),
                            );
                        });
                    });
                ui.add_space(8.0);
            }

            // Welcome Hero when empty
            if self.messages.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(36.0);
                    ui.add(
                        egui::Image::from_bytes(
                            "bytes://garudust-logo",
                            include_bytes!("../../../assets/logo-agent.jpg").as_slice(),
                        )
                        .max_width(56.0)
                        .rounding(10.0),
                    );
                    ui.add_space(10.0);
                    ui.heading(
                        egui::RichText::new("Start a conversation with Garudust AI")
                            .strong()
                            .size(20.0),
                    );
                    ui.label(
                        egui::RichText::new(
                            "Native desktop AI workspace with Local RAG, Terminal, Tools, and MCP support.",
                        )
                        .weak()
                        .size(13.0),
                    );

                    ui.add_space(24.0);

                    // Example Prompt Cards in 2 columns
                    ui.columns(2, |cols| {
                        for (idx, (title, prompt)) in EXAMPLES.iter().enumerate() {
                            let col = &mut cols[idx % 2];
                            let card = egui::Frame::none()
                                .fill(if self.dark {
                                    egui::Color32::from_rgb(26, 28, 38)
                                } else {
                                    egui::Color32::WHITE
                                })
                                .stroke(egui::Stroke::new(
                                    1.0_f32,
                                    if self.dark {
                                        egui::Color32::from_rgb(44, 48, 64)
                                    } else {
                                        egui::Color32::from_rgb(220, 224, 235)
                                    },
                                ))
                                .rounding(10.0)
                                .inner_margin(egui::Margin::symmetric(14.0, 10.0));

                            card.show(col, |ui| {
                                ui.vertical(|ui| {
                                    ui.label(egui::RichText::new(*title).strong().size(13.0));
                                    ui.add_space(2.0);
                                    let btn = ui.add(
                                        egui::Button::new(
                                            egui::RichText::new(*prompt).size(12.0).weak(),
                                        )
                                        .frame(false),
                                    );
                                    if btn.clicked() {
                                        chosen = Some((*prompt).to_string());
                                    }
                                });
                            });
                            col.add_space(8.0);
                        }
                    });

                    ui.add_space(16.0);
                    // Drag and drop hint box
                    egui::Frame::none()
                        .fill(if self.dark {
                            egui::Color32::from_rgb(22, 24, 32)
                        } else {
                            egui::Color32::from_rgb(242, 244, 250)
                        })
                        .stroke(egui::Stroke::new(
                            1.0_f32,
                            egui::Color32::from_rgb(50, 55, 75),
                        ))
                        .rounding(8.0)
                        .inner_margin(egui::Margin::symmetric(16.0, 10.0))
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new("Tip: Drag and drop files (PDF, Markdown, Text, Code) anywhere to perform Semantic RAG search.")
                                    .small()
                                    .weak(),
                            );
                        });
                });
                return;
            }

            // Message Stream
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    let last = self.messages.len().saturating_sub(1);
                    for (idx, m) in self.messages.iter().enumerate() {
                        let working = if !m.is_user
                            && m.content.is_empty()
                            && self.streaming
                            && idx == last
                        {
                            let secs = self.send_at.map_or(0, |t| t.elapsed().as_secs());
                            Some(match &self.active_tool {
                                Some(name) => format!("Running tool: {name}… ({secs}s)"),
                                None => format!("Thinking and responding… ({secs}s)"),
                            })
                        } else {
                            None
                        };

                        bubble(
                            ui,
                            &mut self.md_cache,
                            m.is_user,
                            &m.content,
                            self.dark,
                            working,
                            m.meta.as_deref(),
                        );
                        ui.add_space(8.0);
                    }
                });
        });

        if let Some(text) = chosen {
            self.input = text;
        }
    }

    fn ui_status(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(8.0);
            ui.heading("Status");
            ui.add_space(8.0);
            if ui.button("Refresh").clicked() {
                self.config = ConfigForm::load();
            }
            ui.add_space(8.0);
            egui::Grid::new("status")
                .num_columns(2)
                .spacing([16.0, 8.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Model");
                    ui.label(&self.config.model);
                    ui.end_row();
                    ui.label("Provider");
                    ui.label(&self.config.provider);
                    ui.end_row();
                    ui.label("Base URL");
                    ui.label(if self.config.base_url.is_empty() {
                        "(default)"
                    } else {
                        &self.config.base_url
                    });
                    ui.end_row();
                    ui.label("Approval mode");
                    ui.label(&self.config.approval_mode);
                    ui.end_row();
                    ui.label("Terminal sandbox");
                    ui.label(&self.config.terminal_sandbox);
                    ui.end_row();
                    ui.label("Agent");
                    ui.colored_label(egui::Color32::from_rgb(60, 200, 120), "embedded · ready");
                    ui.end_row();
                    ui.label("Active Session");
                    ui.label(&self.active_session_id);
                    ui.end_row();
                });
        });
    }

    fn ui_config(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add_space(8.0);
                    ui.heading("Config");
                    ui.add_space(8.0);

                    egui::Grid::new("cfg")
                        .num_columns(2)
                        .spacing([12.0, 8.0])
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label("Provider");
                            egui::ComboBox::from_id_salt("provider")
                                .selected_text(&self.config.provider)
                                .show_ui(ui, |ui| {
                                    for p in config_io::PROVIDERS {
                                        if ui
                                            .selectable_value(
                                                &mut self.config.provider,
                                                p.to_string(),
                                                *p,
                                            )
                                            .clicked()
                                        {
                                            let dm = config_io::default_model(p);
                                            if !dm.is_empty() {
                                                self.config.model = dm.to_string();
                                            }
                                        }
                                    }
                                });
                            ui.end_row();

                            ui.label("Model");
                            ui.text_edit_singleline(&mut self.config.model);
                            ui.end_row();

                            // Key hint
                            ui.label("");
                            let key_env = config_io::provider_key_env(&self.config.provider);
                            if key_env.is_empty() {
                                ui.label(
                                    egui::RichText::new("Local provider — no API key needed.")
                                        .small()
                                        .weak(),
                                );
                            } else if self.env_keys.contains(key_env) {
                                ui.label(
                                    egui::RichText::new(format!("✓ {key_env} is set"))
                                        .small()
                                        .color(egui::Color32::from_rgb(60, 200, 120)),
                                );
                            } else {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "⚠ needs {key_env} — set it on Secrets"
                                    ))
                                    .small()
                                    .color(egui::Color32::from_rgb(230, 170, 0)),
                                );
                            }
                            ui.end_row();

                            ui.label("Base URL");
                            ui.text_edit_singleline(&mut self.config.base_url);
                            ui.end_row();
                            ui.label("Reflection model");
                            ui.text_edit_singleline(&mut self.config.reflection_model);
                            ui.end_row();

                            ui.label("Approval mode");
                            egui::ComboBox::from_id_salt("approval")
                                .selected_text(&self.config.approval_mode)
                                .show_ui(ui, |ui| {
                                    for m in config_io::APPROVAL_MODES {
                                        ui.selectable_value(
                                            &mut self.config.approval_mode,
                                            m.to_string(),
                                            *m,
                                        );
                                    }
                                });
                            ui.end_row();

                            ui.label("Terminal sandbox");
                            egui::ComboBox::from_id_salt("sandbox")
                                .selected_text(&self.config.terminal_sandbox)
                                .show_ui(ui, |ui| {
                                    for m in config_io::SANDBOX_MODES {
                                        ui.selectable_value(
                                            &mut self.config.terminal_sandbox,
                                            m.to_string(),
                                            *m,
                                        );
                                    }
                                });
                            ui.end_row();

                            ui.label("Max iterations");
                            ui.add(egui::DragValue::new(&mut self.config.max_iterations));
                            ui.end_row();
                            ui.label("Memory nudge interval");
                            ui.add(egui::DragValue::new(&mut self.config.nudge_interval));
                            ui.end_row();
                            ui.label("Auto-skill threshold");
                            ui.add(egui::DragValue::new(&mut self.config.auto_skill_threshold));
                            ui.end_row();
                            ui.label("Max history pairs");
                            ui.add(egui::DragValue::new(&mut self.config.max_history_pairs));
                            ui.end_row();
                        });

                    ui.add_space(10.0);
                    ui.label(egui::RichText::new("Routing (model hints)").strong());
                    ui.label(
                        egui::RichText::new(
                            "hint → provider/model; shows in the chat Model picker",
                        )
                        .small()
                        .weak(),
                    );
                    let mut remove: Option<usize> = None;
                    for i in 0..self.config.routing.len() {
                        ui.horizontal(|ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.config.routing[i].0)
                                    .desired_width(110.0)
                                    .hint_text("fast"),
                            );
                            ui.label("→");
                            ui.add(
                                egui::TextEdit::singleline(&mut self.config.routing[i].1)
                                    .desired_width(260.0)
                                    .hint_text("groq/llama-3.3-70b-versatile"),
                            );
                            if ui.button("×").clicked() {
                                remove = Some(i);
                            }
                        });
                    }
                    if let Some(i) = remove {
                        self.config.routing.remove(i);
                    }
                    if ui.button("+ Add hint").clicked() {
                        self.config.routing.push((String::new(), String::new()));
                    }

                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if ui.button("Save").clicked() {
                            match self.config.save() {
                                Ok(()) => {
                                    self.config_status = Some("Saved — reloading agent…".into());
                                    self.routing = self.config.routing.clone();
                                    self.default_model = self.config.model.clone();
                                    let _ = self.backend.cmd_tx.send(Cmd::Reload);
                                }
                                Err(e) => self.config_status = Some(format!("error: {e}")),
                            }
                        }
                        if let Some(s) = &self.config_status {
                            ui.label(
                                egui::RichText::new(s)
                                    .small()
                                    .color(egui::Color32::from_rgb(60, 200, 120)),
                            );
                        }
                    });
                    ui.add_space(8.0);
                });
        });
    }

    fn ui_secrets(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add_space(8.0);
                    ui.heading("Secrets");
                    ui.label(
                        egui::RichText::new("Write-only · shown masked · restart to apply")
                            .small()
                            .weak(),
                    );
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.new_key)
                                .hint_text("KEY")
                                .desired_width(220.0),
                        );
                        ui.add(
                            egui::TextEdit::singleline(&mut self.new_value)
                                .password(true)
                                .hint_text("value")
                                .desired_width(220.0),
                        );
                        let ok = !self.new_key.trim().is_empty() && !self.new_value.is_empty();
                        if ui
                            .add_enabled(ok, egui::Button::new("Save secret"))
                            .clicked()
                        {
                            let key = self.new_key.trim().to_uppercase();
                            if !config_io::valid_env_key(&key) {
                                self.secrets_status = Some("invalid key".into());
                            } else {
                                match config_io::set_env(&key, &self.new_value) {
                                    Ok(()) => {
                                        self.secrets_status = Some(format!("Saved {key}"));
                                        self.new_value.clear();
                                        self.new_key.clear();
                                        self.refresh_secrets();
                                    }
                                    Err(e) => self.secrets_status = Some(format!("error: {e}")),
                                }
                            }
                        }
                    });
                    if let Some(s) = &self.secrets_status {
                        ui.label(egui::RichText::new(s).small().weak());
                    }

                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new(format!("Configured ({})", self.secrets.len()))
                            .strong(),
                    );
                    let mut delete: Option<String> = None;
                    for e in &self.secrets {
                        ui.horizontal(|ui| {
                            ui.monospace(&e.key);
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui.button("×").clicked() {
                                        delete = Some(e.key.clone());
                                    }
                                    ui.monospace(&e.masked);
                                },
                            );
                        });
                    }
                    if let Some(k) = delete {
                        let _ = config_io::delete_env(&k);
                        self.refresh_secrets();
                    }
                });
        });
    }
}

fn bubble(
    ui: &mut egui::Ui,
    cache: &mut CommonMarkCache,
    is_user: bool,
    content: &str,
    dark: bool,
    working: Option<String>,
    meta: Option<&str>,
) {
    let is_ingested_card = content.starts_with("**Indexed Document:**");

    let layout = if is_user {
        egui::Layout::right_to_left(egui::Align::TOP)
    } else {
        egui::Layout::left_to_right(egui::Align::TOP)
    };

    ui.with_layout(layout, |ui| {
        let (bg, stroke) = if is_ingested_card {
            (
                if dark {
                    egui::Color32::from_rgb(18, 34, 26)
                } else {
                    egui::Color32::from_rgb(238, 250, 242)
                },
                egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(34, 197, 94)),
            )
        } else if is_user {
            (
                if dark {
                    egui::Color32::from_rgb(46, 42, 33)
                } else {
                    egui::Color32::from_rgb(254, 243, 199)
                },
                egui::Stroke::new(
                    1.0_f32,
                    if dark {
                        egui::Color32::from_rgb(160, 115, 30)
                    } else {
                        egui::Color32::from_rgb(245, 158, 11)
                    },
                ),
            )
        } else if dark {
            (
                egui::Color32::from_rgb(26, 28, 38),
                egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(44, 48, 64)),
            )
        } else {
            (
                egui::Color32::WHITE,
                egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(222, 226, 236)),
            )
        };

        let rounding = if is_user {
            egui::Rounding {
                nw: 14.0,
                ne: 14.0,
                sw: 14.0,
                se: 3.0,
            }
        } else {
            egui::Rounding {
                nw: 14.0,
                ne: 14.0,
                sw: 3.0,
                se: 14.0,
            }
        };

        egui::Frame::none()
            .fill(bg)
            .stroke(stroke)
            .rounding(rounding)
            .inner_margin(egui::Margin::symmetric(14.0, 10.0))
            .show(ui, |ui| {
                ui.set_max_width(ui.available_width() * 0.82);
                ui.vertical(|ui| {
                    // Header with avatar / name
                    ui.horizontal(|ui| {
                        if is_user {
                            ui.label(egui::RichText::new("You").size(11.5).strong().color(
                                if dark {
                                    egui::Color32::from_rgb(225, 175, 75)
                                } else {
                                    egui::Color32::from_rgb(180, 83, 9)
                                },
                            ));
                        } else if is_ingested_card {
                            ui.label(
                                egui::RichText::new("RAG Knowledge Base")
                                    .size(11.5)
                                    .strong()
                                    .color(egui::Color32::from_rgb(34, 197, 94)),
                            );
                        } else {
                            ui.label(
                                egui::RichText::new("Garudust Agent")
                                    .size(11.5)
                                    .strong()
                                    .color(egui::Color32::from_rgb(225, 165, 40)),
                            );
                        }
                    });

                    ui.add_space(4.0);

                    // Body
                    if is_user {
                        ui.label(egui::RichText::new(content).size(13.5).color(if dark {
                            egui::Color32::from_rgb(245, 243, 235)
                        } else {
                            egui::Color32::from_rgb(30, 30, 35)
                        }));
                    } else if let Some(label) = working {
                        ui.horizontal(|ui| {
                            ui.add(egui::Spinner::new().size(14.0));
                            ui.label(
                                egui::RichText::new(label)
                                    .size(13.0)
                                    .color(egui::Color32::from_rgb(225, 165, 40)),
                            );
                        });
                    } else if content.is_empty() {
                        ui.label("…");
                    } else {
                        CommonMarkViewer::new().show(ui, cache, content);

                        // Footer with token/time meta and copy button
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            if let Some(m) = meta {
                                ui.label(egui::RichText::new(format!("⚡ {m}")).small().weak());
                            }

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                egui::RichText::new("Copy").size(11.0),
                                            )
                                            .frame(false),
                                        )
                                        .on_hover_text("Copy message")
                                        .clicked()
                                    {
                                        ui.output_mut(|o| o.copied_text = content.to_owned());
                                    }
                                },
                            );
                        });
                    }
                });
            });
    });
}
