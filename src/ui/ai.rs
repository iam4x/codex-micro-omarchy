use super::{CodexMicro, bindings::ActionTab};
use crate::ai::{self, GeneratedScript, GenerationCancellation, GenerationRequest};
use gpui::{App, Context, Div, Task, Window, div, prelude::*, px, rgb};
use gpui_component::{input::Input, tooltip::Tooltip};

#[derive(Default)]
pub(super) enum AiState {
    #[default]
    Idle,
    Running {
        id: u64,
        prompt: String,
        _cancellation: GenerationCancellation,
        _task: Task<()>,
    },
    Ready {
        prompt: String,
        generated: GeneratedScript,
    },
    Failed(String),
}
impl AiState {
    pub(super) fn status(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running { .. } => "running",
            Self::Ready { .. } => "ready",
            Self::Failed(_) => "failed",
        }
    }
    pub(super) fn output(&self) -> Option<&GeneratedScript> {
        match self {
            Self::Ready { generated, .. } => Some(generated),
            _ => None,
        }
    }
    fn prompt(&self) -> Option<&str> {
        match self {
            Self::Ready { prompt, .. } | Self::Running { prompt, .. } => Some(prompt),
            _ => None,
        }
    }
}

impl CodexMicro {
    pub(super) fn invalidate_ai(&mut self) {
        self.ai_revision = self.ai_revision.wrapping_add(1);
        self.ai_state = AiState::Idle;
    }

    pub(super) fn ai_prompt_changed(&mut self, cx: &mut Context<Self>) {
        let current = self.ai_prompt.read(cx).value();
        if self
            .ai_state
            .prompt()
            .is_some_and(|prompt| prompt != current.as_ref())
            || matches!(self.ai_state, AiState::Failed(_))
        {
            self.invalidate_ai();
        }
        cx.notify();
    }

    pub(super) fn cancel_ai(&mut self, cx: &mut Context<Self>) {
        self.invalidate_ai();
        self.message = "Generation cancelled".into();
        self.message_error = false;
        cx.notify();
    }

    pub(super) fn generate_ai(&mut self, cx: &mut Context<Self>) {
        if self.tab != ActionTab::Ai || matches!(self.ai_state, AiState::Running { .. }) {
            return;
        }
        let prompt = self.ai_prompt.read(cx).value().to_string();
        if prompt.trim().is_empty() {
            self.message = "Describe what this action should do".into();
            self.message_error = true;
            cx.notify();
            return;
        }
        self.invalidate_ai();
        let id = self.ai_revision;
        let request = GenerationRequest {
            prompt: prompt.clone(),
            control: self.selected,
            phase: self.phase,
        };
        let cancellation = GenerationCancellation::default();
        let generation = cx
            .background_executor()
            .spawn(ai::generate(request, cancellation.token()));
        let task = cx.spawn(async move |view, cx| {
            let result = generation.await;
            let _ = view.update(cx, |this, cx| {
                if !matches!(&this.ai_state, AiState::Running { id: active, prompt, .. } if *active == id && this.tab == ActionTab::Ai && prompt == this.ai_prompt.read(cx).value().as_ref()) { return; }
                match result {
                    Ok(generated) => {
                        this.ai_state = AiState::Ready { prompt: this.ai_prompt.read(cx).value().to_string(), generated };
                        this.message = "Script ready. Review it, then save the binding.".into();
                        this.message_error = false;
                    }
                    Err(error) => {
                        let error = format!("{error:#}");
                        this.ai_state = AiState::Failed(error.clone());
                        this.message = error;
                        this.message_error = true;
                    }
                }
                cx.notify();
            });
        });
        self.ai_state = AiState::Running {
            id,
            prompt,
            _cancellation: cancellation,
            _task: task,
        };
        self.message = "Creating your action with Codex…".into();
        self.message_error = false;
        cx.notify();
    }

    pub(super) fn ai_can_run(&self, cx: &App) -> bool {
        matches!(&self.ai_state, AiState::Ready { prompt, .. } if prompt == self.ai_prompt.read(cx).value().as_ref())
    }

    pub(super) fn ai_editor(&self, _window: &mut Window, cx: &mut Context<Self>) -> Div {
        let p = self.palette;
        let running = matches!(self.ai_state, AiState::Running { .. });
        let label = if running {
            "Generating…"
        } else if self.ai_state.output().is_some() {
            "Regenerate action"
        } else {
            "Generate action"
        };
        let mut form = div()
            .w_full()
            .flex()
            .flex_col()
            .gap_2()
            .child(self.label("WHAT SHOULD THIS CONTROL DO?"))
            .child(
                div()
                    .w_full()
                    .flex()
                    .child(Input::new(&self.ai_prompt).h(px(140.)).flex_1().min_w_0()),
            )
            .child(
                div()
                    .mt_1()
                    .text_size(px(12.))
                    .text_color(rgb(p.muted))
                    .child(
                        "Describe the action. Codex creates a script for you to review and save.",
                    ),
            )
            .child(
                div()
                    .mt_2()
                    .flex()
                    .gap_2()
                    .child(
                        div()
                            .id("generate-ai")
                            .flex_1()
                            .h(px(36.))
                            .rounded(px(3.))
                            .border_1()
                            .border_color(rgb(p.accent))
                            .text_color(rgb(p.accent))
                            .text_size(px(12.))
                            .cursor_pointer()
                            .when(running, |button| button.opacity(0.6).cursor_default())
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap_2()
                            .child(self.icon("sparkles", 15., p.accent))
                            .child(label)
                            .tooltip(|window, cx| {
                                Tooltip::new(format!(
                                    "{}: {} with {} reasoning and {} service",
                                    ai::REQUESTED_MODEL,
                                    ai::MODEL,
                                    ai::REASONING,
                                    ai::SERVICE_TIER
                                ))
                                .build(window, cx)
                            })
                            .on_click(cx.listener(|this, _, _, cx| this.generate_ai(cx))),
                    )
                    .when(running, |row| {
                        row.child(
                            div()
                                .id("cancel-ai")
                                .h(px(36.))
                                .px_3()
                                .rounded(px(3.))
                                .border_1()
                                .border_color(rgb(p.border))
                                .text_size(px(12.))
                                .cursor_pointer()
                                .flex()
                                .items_center()
                                .justify_center()
                                .child("Cancel")
                                .on_click(cx.listener(|this, _, _, cx| this.cancel_ai(cx))),
                        )
                    }),
            );
        if let Some(output) = self.ai_state.output() {
            form = form
                .child(
                    div()
                        .mt_3()
                        .text_size(px(12.))
                        .child(output.summary.clone()),
                )
                .child(self.label("GENERATED SCRIPT"))
                .child(
                    div()
                        .w_full()
                        .p_3()
                        .rounded(px(3.))
                        .bg(rgb(p.raised))
                        .text_size(px(11.))
                        .font_family("CaskaydiaMono Nerd Font")
                        .child(output.script.clone()),
                )
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(rgb(p.muted))
                        .child("Save keeps this script. Your button runs it directly."),
                );
        }
        if let AiState::Failed(error) = &self.ai_state {
            form = form.child(
                div()
                    .mt_2()
                    .text_size(px(11.))
                    .text_color(rgb(p.red))
                    .child(error.clone()),
            );
        }
        form
    }
}
