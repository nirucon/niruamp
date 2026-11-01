//! Distraction-free fullscreen playback view.
//!
//! The whole screen is black regardless of the active theme. The track text and
//! CAVA bars use the same NIRUX theme roles as the normal deck, so the view
//! still belongs to the active suite theme without painting a panel.

use crate::app::NiruampApp;
use crate::views::common::fmt_time;
use egui::{pos2, vec2, Align2, Color32, Context, FontId, Rect, Stroke};

impl NiruampApp {
    pub(crate) fn toggle_immersive(&mut self, ctx: &Context) {
        self.immersive = !self.immersive;
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.immersive));
        if !self.immersive && self.prefs.always_on_top {
            ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(
                egui::WindowLevel::AlwaysOnTop,
            ));
        }
        ctx.request_repaint();
    }

    pub(crate) fn draw_immersive(&mut self, ctx: &Context) {
        let palette = self.palette();
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(Color32::BLACK))
            .show(ctx, |ui| {
                let screen = ui.max_rect();
                let painter = ui.painter();
                let scale = (screen.height() / 1000.0).clamp(0.78, 1.75);
                let title_size = (44.0 * scale).min(screen.height() * 0.065);
                let artist_size = (20.0 * scale).min(screen.height() * 0.032);
                let small_size = (12.0 * scale).max(10.0);
                let center = screen.center();
                let title_y = center.y - screen.height() * 0.14;
                let artist_y = title_y + title_size * 0.95;
                let album_y = artist_y + artist_size * 1.35;
                let spec_size = vec2(
                    (screen.width() * 0.54).clamp(360.0, 1500.0),
                    (screen.height() * 0.17).clamp(96.0, 240.0),
                );
                let spec = Rect::from_center_size(
                    pos2(center.x, center.y + screen.height() * 0.10),
                    spec_size,
                );

                let current = self.playback.current.as_ref();
                let title = current
                    .map(|track| track.title.as_str())
                    .filter(|title| !title.is_empty())
                    .unwrap_or("NIRUAMP");
                let artist = current
                    .map(|track| track.artist.as_str())
                    .filter(|artist| !artist.is_empty())
                    .unwrap_or(if current.is_some() {
                        "Unknown artist"
                    } else {
                        "Nothing playing"
                    });
                let album = current
                    .map(|track| track.album.as_str())
                    .filter(|album| !album.is_empty());

                painter.text(
                    pos2(center.x, title_y),
                    Align2::CENTER_CENTER,
                    title,
                    FontId::proportional(title_size),
                    palette.foreground,
                );
                painter.text(
                    pos2(center.x, artist_y),
                    Align2::CENTER_CENTER,
                    artist,
                    FontId::proportional(artist_size),
                    palette.muted,
                );
                if let Some(album) = album {
                    painter.text(
                        pos2(center.x, album_y),
                        Align2::CENTER_CENTER,
                        album,
                        FontId::proportional(small_size),
                        palette.disabled,
                    );
                }

                self.draw_immersive_spectrum(painter, spec, &palette);

                let position = self.position_now();
                let duration = self.playback.duration.max(0.0);
                let progress = if duration > 0.0 {
                    (position / duration).clamp(0.0, 1.0) as f32
                } else {
                    0.0
                };
                let progress_rect = Rect::from_center_size(
                    pos2(center.x, spec.bottom() + screen.height() * 0.035),
                    vec2((screen.width() * 0.28).clamp(260.0, 720.0), 2.0),
                );
                painter.rect_filled(progress_rect, 1.0, palette.border);
                painter.rect_filled(
                    Rect::from_min_size(
                        progress_rect.min,
                        vec2(progress_rect.width() * progress, progress_rect.height()),
                    ),
                    1.0,
                    palette.accent,
                );
                painter.text(
                    pos2(center.x, progress_rect.bottom() + small_size * 1.6),
                    Align2::CENTER_CENTER,
                    format!("{} / {}", fmt_time(position), fmt_time(duration)),
                    FontId::monospace(small_size),
                    palette.disabled,
                );

                painter.text(
                    pos2(center.x, screen.bottom() - 34.0 * scale),
                    Align2::CENTER_CENTER,
                    "Press Esc or F11 to close fullscreen",
                    FontId::proportional(small_size),
                    palette.disabled,
                );
            });
    }

    fn draw_immersive_spectrum(
        &self,
        painter: &egui::Painter,
        rect: Rect,
        palette: &crate::theme::Palette,
    ) {
        let inner = rect;
        let baseline = inner.bottom();
        painter.line_segment(
            [pos2(inner.left(), baseline), pos2(inner.right(), baseline)],
            Stroke::new(1.0_f32, palette.border),
        );
        if !self.capabilities.visualization {
            painter.text(
                inner.center(),
                Align2::CENTER_CENTER,
                "CAVA NOT INSTALLED",
                FontId::proportional(12.0),
                palette.disabled,
            );
            return;
        }
        if !self.vis.failed.is_empty() {
            painter.text(
                inner.center(),
                Align2::CENTER_CENTER,
                "VISUALIZER UNAVAILABLE",
                FontId::proportional(12.0),
                palette.warning,
            );
            return;
        }
        if self.vis.displayed.is_empty() {
            let label = if self.playback.is_playing() {
                "ACQUIRING CAVA"
            } else {
                "PLAYBACK IS STOPPED"
            };
            painter.text(
                inner.center(),
                Align2::CENTER_CENTER,
                label,
                FontId::proportional(12.0),
                palette.disabled,
            );
            return;
        }
        let n = self.vis.displayed.len();
        let gap = 2.0;
        let bar_width =
            ((inner.width() - gap * n.saturating_sub(1) as f32) / n.max(1) as f32).max(1.0);
        for (i, value) in self.vis.displayed.iter().take(n).enumerate() {
            let value = value.clamp(0.0, 1.0);
            let height = (inner.height() * (0.04 + value * 0.96)).max(1.0);
            let x = inner.left() + i as f32 * (bar_width + gap);
            let bar = Rect::from_min_max(
                pos2(x, inner.bottom() - height),
                pos2(x + bar_width, inner.bottom()),
            );
            let color = if value > 0.82 {
                palette.vis_peak
            } else if value > 0.45 {
                palette.vis
            } else {
                palette.led_dim
            };
            painter.rect_filled(bar, 1.0, color);
            if value > 0.18 {
                painter.line_segment(
                    [bar.left_top(), bar.right_top()],
                    Stroke::new(1.0_f32, palette.accent),
                );
            }
        }
    }
}
