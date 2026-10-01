use super::{CodexMicro, control};
use crate::daemon;
use gpui::{Context, Window};
use std::time::Duration;

impl CodexMicro {
    pub(super) fn start_events(window: &mut Window, cx: &mut Context<Self>) {
        let receiver = daemon::events();
        let control_queue = match control::receiver() {
            Ok(queue) => queue,
            Err(error) => {
                eprintln!("UI control endpoint: {error}");
                None
            }
        };
        cx.spawn_in(window, async move |view, cx| {
            loop {
                smol::Timer::after(Duration::from_millis(60)).await;
                if let Some(queue) = &control_queue {
                    control::poll(queue, &view, cx);
                }
                if view
                    .update_in(cx, |this, window, cx| {
                        let mut changed = false;
                        for event in receiver.try_iter() {
                            this.receive(event, window, cx);
                            changed = true;
                        }
                        if this
                            .active
                            .is_some_and(|(_, time)| time.elapsed() > Duration::from_millis(600))
                        {
                            this.active = None;
                            changed = true;
                        }
                        if changed {
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
    }
}
