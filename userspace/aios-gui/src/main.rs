//! Lumen — integrated desktop shell for ai-native-os (M4/M24).
//!
//! A single window with a top bar, a dock, and three switchable panels:
//! Command bar (real local inference), Activity center (live governor
//! decisions), and System monitor (real host resources). Inference runs off
//! the UI thread. Colors follow the Lumen design tokens.
use fltk::{
    app,
    button::Button,
    enums::{Align, Color, Font, FrameType},
    frame::Frame,
    group::Group,
    input::Input,
    prelude::*,
    text::{TextBuffer, TextDisplay},
    window::Window,
};
use libagent::{Agent, ResourceLimits, TrustLevel};
use libgovernor::{evaluate, Decision};
use libintent::{ApprovalPolicy, Intent, RiskLevel};
use libprovenance::ProvenanceLog;
use libprovider::{AIProvider, ChatRequest, LlamaProvider, LocalProvider, Message};
use libresource::{probe_host, ResourceKind};

const BG: (u8, u8, u8) = (14, 17, 22);
const SURFACE: (u8, u8, u8) = (22, 27, 34);
const ACCENT: (u8, u8, u8) = (110, 155, 255);
const OKG: (u8, u8, u8) = (63, 203, 126);
const TXT: (u8, u8, u8) = (230, 234, 240);

#[derive(Clone)]
enum Msg {
    Show(u8),
    Ask,
    Result(String, String),
}

fn infer(prompt: &str) -> (String, String) {
    let req = ChatRequest {
        model: "local".into(),
        system: None,
        messages: vec![Message {
            role: "user".into(),
            content: prompt.into(),
        }],
        max_tokens: 200,
    };
    if let Some(l) = LlamaProvider::from_env() {
        if let Ok(r) = l.chat(&req) {
            let gpu = std::env::var("AIOS_LLAMA_NGL")
                .ok()
                .as_deref()
                .unwrap_or("0")
                != "0";
            return (
                if gpu {
                    "● Local AI · GPU (llama.cpp)".into()
                } else {
                    "● Local AI · CPU (llama.cpp)".into()
                },
                r.text,
            );
        }
    }
    (
        "● Local AI (stub)".into(),
        LocalProvider::new().chat(&req).unwrap().text,
    )
}

fn activity_text() -> String {
    let now = 2000;
    let a = Agent::create(
        "user:harsh",
        "assistant",
        "local/reasoning",
        TrustLevel::Standard,
        ResourceLimits::default(),
        10_000,
    );
    let mut i = Intent::create(
        "user:harsh",
        "Summarize notes",
        "project + local model",
        vec![
            "fs.read:/home/harsh/project".into(),
            "model.invoke:local/reasoning".into(),
            "net.connect:api.example.com:443".into(),
        ],
        RiskLevel::Medium,
        ApprovalPolicy::AtOrAbove(RiskLevel::High),
        ResourceLimits::default(),
        now,
        10_000,
    );
    i.validate().unwrap();
    i.authorize().unwrap();
    i.begin_execution().unwrap();
    let mut p = ProvenanceLog::new();
    let mut out = format!(
        "Agent {}…  intent {}  [{:?}]\n\n",
        &a.id()[..12],
        i.intent_id,
        i.state
    );
    for act in [
        "fs.read:/home/harsh/project/README.md",
        "model.invoke:local/reasoning",
        "net.connect:api.example.com:443",
        "net.connect:evil.com:443",
    ] {
        let d = evaluate(&a, &i, act, None, now, &mut p);
        let tag = match &d {
            Decision::Allow => "ALLOW ",
            Decision::NeedApproval { .. } => "APPROVE",
            Decision::Deny { .. } => "BLOCK ",
        };
        out.push_str(&format!("[{tag}] {act}\n"));
    }
    out.push_str(&format!(
        "\nProvenance: {} events, chain valid: {}",
        p.len(),
        p.verify().is_ok()
    ));
    out
}

fn monitor_text() -> String {
    let mut s = String::from("Resources (AI workloads first-class)\n\n");
    for r in probe_host() {
        let used = match r.kind {
            ResourceKind::Ram => format!(
                "{:.1}/{:.1} GiB",
                (r.capacity - r.available) as f64 / 1e9,
                r.capacity as f64 / 1e9
            ),
            _ => format!("{}/{} {}", r.capacity - r.available, r.capacity, r.unit),
        };
        s.push_str(&format!("  {:<6}  {}\n", format!("{:?}", r.kind), used));
    }
    s
}

fn main() {
    let a = app::App::default();
    app::background(BG.0, BG.1, BG.2);
    let mut win = Window::default()
        .with_size(720, 520)
        .with_label("Lumen — ai-native-os");
    win.set_color(Color::from_rgb(BG.0, BG.1, BG.2));

    // top bar
    let mut title = Frame::new(16, 10, 300, 26, "◆ Lumen");
    title.set_label_font(Font::HelveticaBold);
    title.set_label_size(18);
    title.set_label_color(Color::from_rgb(ACCENT.0, ACCENT.1, ACCENT.2));
    title.set_align(Align::Left | Align::Inside);
    let mut sysind = Frame::new(520, 10, 184, 26, "● Local AI");
    sysind.set_label_color(Color::from_rgb(OKG.0, OKG.1, OKG.2));
    sysind.set_align(Align::Right | Align::Inside);

    // dock
    let labels = ["Ask", "Activity", "Monitor"];
    let (s, r) = app::channel::<Msg>();
    for (idx, lb) in labels.iter().enumerate() {
        let mut b = Button::new(16 + idx as i32 * 92, 44, 84, 30, *lb);
        b.set_color(Color::from_rgb(30, 36, 45));
        b.set_label_color(Color::from_rgb(TXT.0, TXT.1, TXT.2));
        b.set_frame(FrameType::RFlatBox);
        b.emit(s, Msg::Show(idx as u8));
    }

    // ---- panel: command bar ----
    let mut g_ask = Group::new(16, 86, 688, 420, "");
    let mut input = Input::new(16, 92, 560, 38, "");
    input.set_color(Color::from_rgb(30, 36, 45));
    input.set_text_color(Color::from_rgb(TXT.0, TXT.1, TXT.2));
    input.set_text_size(15);
    input.set_frame(FrameType::RFlatBox);
    input.set_value("What is an AI-native operating system?");
    let mut ask = Button::new(584, 92, 120, 38, "Ask");
    ask.set_color(Color::from_rgb(59, 110, 245));
    ask.set_label_color(Color::White);
    ask.set_frame(FrameType::RFlatBox);
    ask.emit(s, Msg::Ask);
    let mut abuf = TextBuffer::default();
    abuf.set_text("Ask anything. Runs locally on your machine.");
    let mut adisp = TextDisplay::new(16, 140, 688, 366, "");
    adisp.set_buffer(abuf.clone());
    adisp.set_color(Color::from_rgb(SURFACE.0, SURFACE.1, SURFACE.2));
    adisp.set_text_color(Color::from_rgb(TXT.0, TXT.1, TXT.2));
    adisp.set_frame(FrameType::RFlatBox);
    g_ask.end();

    // ---- panel: activity ----
    let mut g_act = Group::new(16, 86, 688, 420, "");
    let mut actbuf = TextBuffer::default();
    actbuf.set_text(&activity_text());
    let mut actdisp = TextDisplay::new(16, 92, 688, 414, "");
    actdisp.set_buffer(actbuf);
    actdisp.set_color(Color::from_rgb(SURFACE.0, SURFACE.1, SURFACE.2));
    actdisp.set_text_color(Color::from_rgb(TXT.0, TXT.1, TXT.2));
    actdisp.set_frame(FrameType::RFlatBox);
    g_act.end();
    g_act.hide();

    // ---- panel: monitor ----
    let mut g_mon = Group::new(16, 86, 688, 420, "");
    let mut monbuf = TextBuffer::default();
    monbuf.set_text(&monitor_text());
    let mut mondisp = TextDisplay::new(16, 92, 688, 414, "");
    mondisp.set_buffer(monbuf);
    mondisp.set_color(Color::from_rgb(SURFACE.0, SURFACE.1, SURFACE.2));
    mondisp.set_text_color(Color::from_rgb(TXT.0, TXT.1, TXT.2));
    mondisp.set_frame(FrameType::RFlatBox);
    g_mon.end();
    g_mon.hide();

    win.end();
    win.show();

    // Optional start panel (for deterministic screenshots / deep-linking).
    match std::env::args().nth(1).as_deref() {
        Some("activity") => {
            g_ask.hide();
            g_act.show();
            g_mon.hide();
        }
        Some("monitor") => {
            g_ask.hide();
            g_act.hide();
            g_mon.show();
        }
        _ => {}
    }

    let sender = app::Sender::<Msg>::get();
    while a.wait() {
        if let Some(m) = r.recv() {
            match m {
                Msg::Show(0) => {
                    g_ask.show();
                    g_act.hide();
                    g_mon.hide();
                }
                Msg::Show(1) => {
                    g_ask.hide();
                    g_act.show();
                    g_mon.hide();
                }
                Msg::Show(_) => {
                    g_ask.hide();
                    g_act.hide();
                    g_mon.show();
                }
                Msg::Ask => {
                    let prompt = input.value();
                    sysind.set_label("… thinking");
                    app::flush();
                    let sc = sender;
                    std::thread::spawn(move || {
                        let (i, t) = infer(&prompt);
                        sc.send(Msg::Result(i, t));
                        app::awake();
                    });
                }
                Msg::Result(ind, text) => {
                    let gpu = ind.contains("GPU");
                    sysind.set_label(&ind);
                    sysind.set_label_color(if gpu {
                        Color::from_rgb(ACCENT.0, ACCENT.1, ACCENT.2)
                    } else {
                        Color::from_rgb(OKG.0, OKG.1, OKG.2)
                    });
                    abuf.set_text(&text);
                    app::flush();
                }
            }
        }
    }
}
