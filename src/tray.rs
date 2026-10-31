//! Optional StatusNotifierItem tray integration.
//!
//! NIRUX/NIRUBAR speaks the freedesktop StatusNotifierItem protocol. The tray is
//! best-effort: a missing D-Bus session or watcher must never prevent NIRUAMP
//! from running. All menu actions are forwarded to the GUI thread over a small
//! bounded command channel.
use egui::Context;
use ksni::blocking::{Handle, TrayMethods};
use ksni::menu::{MenuItem, StandardItem};
use ksni::{Icon, ToolTip};
use std::sync::mpsc::Sender;

#[derive(Debug)]
pub enum TrayCommand {
    Show,
    Toggle,
    Next,
    Previous,
    Quit,
    Volume(i32),
}

pub struct TrayHandler {
    tx: Sender<TrayCommand>,
    ctx: Context,
    title: String,
    detail: String,
    playing: bool,
}

impl TrayHandler {
    fn send(&self, command: TrayCommand) {
        let _ = self.tx.send(command);
        self.ctx.request_repaint();
    }
}

impl ksni::Tray for TrayHandler {
    fn id(&self) -> String {
        "niruamp".into()
    }

    fn title(&self) -> String {
        if self.title.is_empty() {
            "NIRUAMP".into()
        } else {
            format!("NIRUAMP · {}", self.title)
        }
    }

    fn icon_name(&self) -> String {
        "niruamp".into()
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        vec![tray_icon()]
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            icon_name: "niruamp".into(),
            icon_pixmap: vec![tray_icon()],
            title: self.title().to_owned(),
            description: self.detail.clone(),
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        self.send(TrayCommand::Show);
    }

    fn secondary_activate(&mut self, _x: i32, _y: i32) {
        self.send(TrayCommand::Toggle);
    }

    fn scroll(&mut self, delta: i32, _orientation: ksni::Orientation) {
        if delta != 0 {
            self.send(TrayCommand::Volume(delta.signum() * 5));
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let play_label = if self.playing { "Pause" } else { "Play" };
        vec![
            StandardItem {
                label: "Show NIRUAMP".into(),
                icon_name: "view-restore".into(),
                activate: Box::new(|this: &mut Self| this.send(TrayCommand::Show)),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: play_label.into(),
                icon_name: if self.playing {
                    "media-playback-pause"
                } else {
                    "media-playback-start"
                }
                .into(),
                activate: Box::new(|this: &mut Self| this.send(TrayCommand::Toggle)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Next track".into(),
                icon_name: "media-skip-forward".into(),
                activate: Box::new(|this: &mut Self| this.send(TrayCommand::Next)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Previous track".into(),
                icon_name: "media-skip-backward".into(),
                activate: Box::new(|this: &mut Self| this.send(TrayCommand::Previous)),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit NIRUAMP".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|this: &mut Self| this.send(TrayCommand::Quit)),
                ..Default::default()
            }
            .into(),
        ]
    }
}

pub struct TrayHandle {
    inner: Handle<TrayHandler>,
    title: String,
    detail: String,
    playing: bool,
}

impl TrayHandle {
    pub fn spawn(tx: Sender<TrayCommand>, ctx: Context) -> Result<Self, String> {
        let handler = TrayHandler {
            tx,
            ctx,
            title: String::new(),
            detail: "NIRUAMP · NPLAY frontend".into(),
            playing: false,
        };
        let inner = handler
            .assume_sni_available(true)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            inner,
            title: String::new(),
            detail: String::new(),
            playing: false,
        })
    }

    pub fn update(&mut self, title: &str, detail: &str, playing: bool) {
        if title == self.title && detail == self.detail && playing == self.playing {
            return;
        }
        self.title = title.to_string();
        self.detail = detail.to_string();
        self.playing = playing;
        let title = self.title.clone();
        let detail = self.detail.clone();
        let playing = self.playing;
        self.inner.update(move |tray| {
            tray.title = title;
            tray.detail = detail;
            tray.playing = playing;
        });
    }

    pub fn shutdown(&self) {
        self.inner.shutdown().wait();
    }
}

/// Original 64×64 tray icon, ARGB32 network byte order.
fn tray_icon() -> Icon {
    let size = 64usize;
    let mut data = vec![0u8; size * size * 4];
    let mut put = |x: usize, y: usize, rgba: [u8; 4]| {
        let i = (y * size + x) * 4;
        data[i] = rgba[3];
        data[i + 1] = rgba[0];
        data[i + 2] = rgba[1];
        data[i + 3] = rgba[2];
    };
    for y in 0..size {
        for x in 0..size {
            let edge = x < 3 || y < 3 || x >= size - 3 || y >= size - 3;
            let radius = 9;
            let corner = (x < radius
                && y < radius
                && (radius - x).pow(2) + (radius - y).pow(2) > radius * radius)
                || (x >= size - radius
                    && y < radius
                    && (x - (size - radius - 1)).pow(2) + (radius - y).pow(2) > radius * radius)
                || (x < radius
                    && y >= size - radius
                    && (radius - x).pow(2) + (y - (size - radius - 1)).pow(2) > radius * radius)
                || (x >= size - radius
                    && y >= size - radius
                    && (x - (size - radius - 1)).pow(2) + (y - (size - radius - 1)).pow(2)
                        > radius * radius);
            if corner {
                continue;
            }
            let color = if edge {
                [58, 61, 63, 255]
            } else if y < 18 {
                [38, 40, 42, 255]
            } else if y >= size - 18 {
                [16, 17, 18, 255]
            } else {
                [26, 28, 29, 255]
            };
            put(x, y, color);
        }
    }
    // Amber play mark.
    for y in 22..43 {
        let span = (y as i32 - 22).min(42 - y as i32).max(0) as usize;
        for x in 21..(21 + 4 + span) {
            if x < size - 20 {
                put(x, y, [214, 168, 95, 255]);
            }
        }
    }
    // Green spectrum/checkbox mark.
    for (i, h) in [5usize, 9, 6, 11, 7, 4].into_iter().enumerate() {
        for y in (size - 8 - h)..(size - 8) {
            for x in (22 + i * 4)..(24 + i * 4) {
                put(x, y, [142, 192, 124, 255]);
            }
        }
    }
    Icon {
        width: size as i32,
        height: size as i32,
        data,
    }
}
