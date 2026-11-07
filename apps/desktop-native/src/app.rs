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
const EXAMPLES: [&str; 4] = [
    "สรุป git log ล่าสุดของโปรเจกต์นี้",
    "ค้นเว็บ: ข่าว AI agent ล่าสุด",
    "อ่านไฟล์ README.md แล้วสรุปให้หน่อย",
    "ช่วยเขียนสคริปต์ backup โฟลเดอร์",
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
}

impl App {
    pub fn new(
        backend: Backend,
        boot_ms: u128,
        dark: bool,
        font_level: usize,
        saved_sessions: Option<Vec<SessionItem>>,
        saved_active_session: Option<String>,
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

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Apply appearance (idempotent — egui no-ops when unchanged).
        ctx.set_visuals(if self.dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });
        ctx.set_zoom_factor(FONT_ZOOM[self.font_level]);

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
                                    "{total} tokens (↑{} ↓{}) · {secs:.1}s",
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
                            "📄 **Indexed Document:** `{file_name}` ({chunks} chunks)\n\nYou can now ask questions or search across this document using semantic RAG!"
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

        egui::SidePanel::left("nav")
            .exact_width(190.0)
            .resizable(false)
            .show(ctx, |ui| {
                ui.add_space(10.0);
                // Logo banner above the name.
                ui.add(
                    egui::Image::from_bytes(
                        "bytes://garudust-logo",
                        include_bytes!("../../../assets/logo-agent.jpg").as_slice(),
                    )
                    .max_width(ui.available_width())
                    .rounding(4.0),
                );
                ui.add_space(2.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new(format!("native · {} ms", self.boot_ms))
                            .small()
                            .weak(),
                    )
                });
                ui.separator();
                ui.add_space(4.0);
                for (p, label) in [
                    (Page::Chat, "💬 Chat"),
                    (Page::Status, "📊 Status"),
                    (Page::Config, "⚙ Config"),
                    (Page::Secrets, "🔒 Secrets"),
                ] {
                    if ui.selectable_label(self.page == p, label).clicked() {
                        self.page = p;
                        if p == Page::Secrets {
                            self.refresh_secrets();
                        }
                    }
                }

                // If on Chat page, show Sessions Sidebar
                if self.page == Page::Chat {
                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Chats").strong());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .button("+ New")
                                .on_hover_text("Start a new chat session")
                                .clicked()
                                && !self.streaming
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
                    ui.add_space(4.0);

                    let mut switch_to: Option<String> = None;
                    let mut delete_sess: Option<usize> = None;

                    egui::ScrollArea::vertical()
                        .max_height(200.0)
                        .show(ui, |ui| {
                            for (i, s) in self.sessions.iter().enumerate() {
                                let is_active = self.active_session_id == s.id;
                                ui.horizontal(|ui| {
                                    let btn = ui.selectable_label(is_active, &s.title);
                                    if btn.clicked() && !is_active && !self.streaming {
                                        switch_to = Some(s.id.clone());
                                    }
                                    if self.sessions.len() > 1 {
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                if ui
                                                    .small_button("×")
                                                    .on_hover_text("Delete chat")
                                                    .clicked()
                                                    && !self.streaming
                                                {
                                                    delete_sess = Some(i);
                                                }
                                            },
                                        );
                                    }
                                });
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

                // Appearance controls, pinned to the bottom.
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        for (i, label) in FONT_LABELS.iter().enumerate() {
                            ui.selectable_value(&mut self.font_level, i, *label);
                        }
                    });
                    ui.label(egui::RichText::new("Font size").small().weak());
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.dark, true, "Dark");
                        ui.selectable_value(&mut self.dark, false, "Light");
                    });
                    ui.label(egui::RichText::new("Theme").small().weak());
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

    // Persist appearance prefs and sessions (window geometry is persisted by eframe itself).
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

        egui::TopBottomPanel::bottom("input").show(ctx, |ui| {
            ui.add_space(4.0);

            // Ingest file bar if toggled
            if self.show_ingest_bar {
                ui.horizontal(|ui| {
                    ui.label("📄 Ingest file:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.ingest_input)
                            .hint_text("Absolute path to PDF, TXT, MD, CSV, Code...")
                            .desired_width(ui.available_width() - 80.0),
                    );
                    if ui.button("Index").clicked() && !self.ingest_input.trim().is_empty() {
                        let path = self.ingest_input.trim().to_string();
                        self.ingest_input.clear();
                        self.show_ingest_bar = false;
                        let _ = self.backend.cmd_tx.send(Cmd::IngestFile {
                            session_key: self.active_session_id.clone(),
                            path,
                        });
                    }
                });
                ui.add_space(4.0);
            }

            // Model picker + Ingest toggle
            ui.horizontal(|ui| {
                ui.label("Model:");
                let sel = if self.hint.is_empty() {
                    format!("Default · {}", self.default_model)
                } else {
                    self.hint.clone()
                };
                egui::ComboBox::from_id_salt("model")
                    .selected_text(sel)
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
                if self.routing.is_empty() {
                    ui.label(egui::RichText::new("(add hints in Config)").small().weak());
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("📎 Ingest file").on_hover_text("Index a local file into RAG (or drag & drop anywhere)").clicked() {
                        self.show_ingest_bar = !self.show_ingest_bar;
                    }
                });
            });

            // Text + send/stop. Multiline so prompts can span lines; Enter sends,
            // Shift+Enter inserts a newline. The newline that the multiline widget
            // inserts on a plain Enter is harmless — send() trims and clears it.
            ui.horizontal(|ui| {
                let btn_w = 64.0;
                let resp = ui.add_enabled(
                    !self.streaming,
                    egui::TextEdit::multiline(&mut self.input)
                        .hint_text("Message Garudust…  (Enter ส่ง · Shift+Enter ขึ้นบรรทัด · ลากไฟล์มาวางเพื่อทำ RAG)")
                        .desired_rows(1)
                        .desired_width(ui.available_width() - btn_w),
                );
                let enter = resp.has_focus()
                    && ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift);
                if self.streaming {
                    if ui.button("Stop").clicked() {
                        let _ = self.backend.cmd_tx.send(Cmd::Stop);
                    }
                } else if ui.button("Send").clicked() || enter {
                    self.send();
                    resp.request_focus();
                }
            });
            ui.add_space(4.0);
        });

        // Example prompt chosen on the empty screen; applied after the closure
        // to avoid borrowing `self.input` while `self` is borrowed by the panel.
        let mut chosen: Option<String> = None;
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.messages.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(48.0);
                    ui.heading("เริ่มสนทนากับ Garudust");
                    ui.label(
                        egui::RichText::new("เลือกตัวอย่างด้านล่าง หรือลากไฟล์มาวางเพื่อทำ RAG ค้นหาข้อมูล")
                            .weak(),
                    );
                    ui.add_space(16.0);
                    for ex in EXAMPLES {
                        if ui.button(ex).clicked() {
                            chosen = Some(ex.to_string());
                        }
                        ui.add_space(4.0);
                    }
                });
                return;
            }
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    let last = self.messages.len().saturating_sub(1);
                    for (idx, m) in self.messages.iter().enumerate() {
                        // The trailing assistant bubble shows a live working
                        // indicator until its first token arrives.
                        let working = if !m.is_user
                            && m.content.is_empty()
                            && self.streaming
                            && idx == last
                        {
                            let secs = self.send_at.map_or(0, |t| t.elapsed().as_secs());
                            Some(match &self.active_tool {
                                Some(name) => format!("🔧 {name}…  ({secs}s)"),
                                None => format!("กำลังคิด…  ({secs}s)"),
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
                        );
                        if let Some(meta) = &m.meta {
                            ui.label(egui::RichText::new(meta).small().weak());
                        }
                        ui.add_space(6.0);
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
                            if ui.button("✕").clicked() {
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
                                    if ui.button("✕").clicked() {
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
) {
    let layout = if is_user {
        egui::Layout::right_to_left(egui::Align::TOP)
    } else {
        egui::Layout::left_to_right(egui::Align::TOP)
    };
    ui.with_layout(layout, |ui| {
        let bg = if is_user {
            egui::Color32::from_rgb(217, 159, 0)
        } else if dark {
            egui::Color32::from_rgb(40, 40, 46)
        } else {
            egui::Color32::from_rgb(230, 230, 236)
        };
        egui::Frame::none()
            .fill(bg)
            .rounding(10.0)
            .inner_margin(egui::Margin::symmetric(10.0, 6.0))
            .show(ui, |ui| {
                ui.set_max_width(ui.available_width() * 0.8);
                if is_user {
                    ui.colored_label(egui::Color32::BLACK, content);
                } else if let Some(label) = working {
                    ui.horizontal(|ui| {
                        ui.add(egui::Spinner::new().size(14.0));
                        ui.label(egui::RichText::new(label).weak());
                    });
                } else if content.is_empty() {
                    ui.label("…");
                } else {
                    CommonMarkViewer::new().show(ui, cache, content);
                    // Copy the raw markdown of this reply.
                    if ui.small_button("📋").on_hover_text("คัดลอกข้อความ").clicked()
                    {
                        ui.output_mut(|o| o.copied_text = content.to_owned());
                    }
                }
            });
    });
}
