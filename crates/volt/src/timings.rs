use std::sync::{Mutex, Arc};
use lazy_static::lazy_static;

pub fn now_ns() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos() as f64
}

pub fn ns_to_ms(ns: f64) -> f64 {
    ns / 1_000_000.0
}

macro_rules! generate_timings {
    ($($name:ident),*) => {
        struct SharedTimings {
            $(
                $name: f64,
            )*
        }

        lazy_static! {
            static ref SHARED_TIMINGS: Arc<Mutex<SharedTimings>> = Arc::new(Mutex::new(SharedTimings {
                $(
                    $name: 0.0,
                )*
            }));
        }

        $(
            paste::item! {
                #[allow(dead_code)]
                pub fn [<get_ $name _time>]() -> f64 {
                    SHARED_TIMINGS.lock().unwrap().$name
                }

                #[allow(dead_code)]
                pub fn [<set_ $name _time>](time: f64) {
                    SHARED_TIMINGS.lock().unwrap().$name = time;
                }
            }
        )*

        #[allow(dead_code)]
        pub fn show_timings(ctx: &egui::Context, window_name: &str, accuracy: usize) {
            egui::Window::new(window_name)
                .collapsible(false)
                .show(ctx, |ui| {
                    $(
                        paste::item! {
                            ui.label(format!("{}: {:.accuracy$}ms", stringify!($name), ns_to_ms([<get_ $name _time>]()), accuracy = accuracy));
                        }
                    )*
                });
        }
    };
}

generate_timings!(
    render
);