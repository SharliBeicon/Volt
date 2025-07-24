use std::time::Duration;

use egui::Color32;

use crate::timings::now_ns;

#[derive(Debug, Clone)]
pub struct Notification {
    pub message: String,
    pub duration: Option<Duration>,
    pub add_time: Duration
}

impl Notification {
    pub fn new(message: String, duration: Option<Duration>) -> Self {
        let add_time = Duration::from_nanos(now_ns() as u64);
        Notification {
            message,
            duration,
            add_time
        }
    }

    pub fn with_duration(message: String, duration: Duration) -> Self {
        Notification::new(message, Some(duration))
    }

    pub fn without_duration(message: String) -> Self {
        Notification::new(message, None)
    }
}

pub struct NotificationDrawer {
    notifications: Vec<Notification>,
}

impl NotificationDrawer {
    pub fn new() -> Self {
        NotificationDrawer {
            notifications: Vec::new(),
        }
    }

    pub fn add_notification(&mut self, notification: Notification) {
        self.notifications.push(notification);
    }

    pub fn remove_notification(&mut self, index: usize) {
        if index < self.notifications.len() {
            self.notifications.remove(index);
        }
    }

    pub fn get_notifications(&self) -> &Vec<Notification> {
        &self.notifications
    }

    pub fn make(&mut self, message: String, duration: Option<Duration>) {
        let notification = Notification::new(message, duration);
        self.add_notification(notification);
    }
}

impl egui::Widget for &mut NotificationDrawer {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let mut response = ui.allocate_response(egui::Vec2::ZERO, egui::Sense::hover());

        if !self.notifications.is_empty() {
            let now = now_ns() as u64;
            let mut indices_to_remove = Vec::new();

            for (i, notification) in self.notifications.iter().enumerate() {
                let age = now - notification.add_time.as_nanos() as u64;
                let fade_duration_ns = 0.2 * 1_000_000_000.0;
                let lifetime_ns = notification.duration.map(|d| d.as_nanos() as f64).unwrap_or(f64::MAX);
                let mut opacity: f32 = if age as f64 <= fade_duration_ns {
                    (age as f64 / fade_duration_ns) as f32
                } else if lifetime_ns - age as f64 <= fade_duration_ns {
                    ((lifetime_ns - age as f64) / fade_duration_ns) as f32
                } else {
                    1.0
                };

                if opacity <= 0.0 {
                    opacity = 0.01;
                }

                let color = Color32::from_hex("#222222").unwrap().gamma_multiply(opacity);

                egui::Frame::none().fill(color).inner_margin(egui::Margin::same(10.)).show(ui, |ui| {
                    let width = ui.ctx().screen_rect().width();
                    let min_width = if width < 200. {
                        width
                    } else {
                        200.
                    };
                    ui.set_min_width(min_width);
                    ui.allocate_ui(ui.available_size(), |ui| {
                        let text_color = Color32::WHITE.gamma_multiply(opacity);
                        ui.label(egui::RichText::new(&notification.message).color(text_color));
                    });
                });

                // Schedule removal if a duration is specified
                if let Some(duration) = notification.duration {
                    if (notification.add_time.as_nanos() as u64) + (duration.as_nanos() as u64) < now {
                        indices_to_remove.push(i);
                    }
                }

                ui.ctx().request_repaint_after_secs(0.03);
            }

            // Remove notifications in reverse order to avoid index invalidation
            for index in indices_to_remove.into_iter().rev() {
                self.remove_notification(index);
            }

            response = ui.allocate_response(ui.available_size(), egui::Sense::hover());
        }

        response
    }
}