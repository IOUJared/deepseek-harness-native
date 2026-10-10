//! Native fixed-field runtime controls; no provider, account or generation requests.
use super::*;
use dsh_native_core::{CodexCacheRetention, CodexRuntimeTransport, CodexRuntimeValues};
use iced::widget::{Column, column, pick_list};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Transport(Option<CodexRuntimeTransport>);
impl std::fmt::Display for Transport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            self.0
                .map_or("Default / inherit", CodexRuntimeTransport::as_str),
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Cache(Option<CodexCacheRetention>);
impl std::fmt::Display for Cache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            self.0
                .map_or("Default / inherit", CodexCacheRetention::as_str),
        )
    }
}
fn menu(_: &iced::Theme) -> iced::widget::overlay::menu::Style {
    iced::widget::overlay::menu::Style {
        background: design::OVERLAY.into(),
        text_color: TEXT,
        selected_text_color: ACCENT,
        selected_background: design::BASE.into(),
        border: iced::Border {
            color: design::LINE,
            width: 1.0,
            radius: 9.0.into(),
        },
        shadow: iced::Shadow::default(),
    }
}
fn numeric_hint(values: CodexRuntimeValues, field: RuntimeField) -> String {
    match field {
        RuntimeField::Timeout => values.timeout_ms.map(|v| v.get().to_string()),
        RuntimeField::WebsocketConnectTimeout => values
            .websocket_connect_timeout_ms
            .map(|v| v.get().to_string()),
        RuntimeField::StreamIdleTimeout => values
            .stream_idle_timeout_ms
            .map(|v| v.as_f64().to_string()),
        RuntimeField::ImagePixelBudget => values
            .request_image_pixel_budget
            .map(|v| v.get().to_string()),
        RuntimeField::ImageMaxBytes => values.request_image_max_bytes.map(|v| v.get().to_string()),
        RuntimeField::MaxRequestImageBytes => {
            values.max_request_image_bytes.map(|v| v.get().to_string())
        }
    }
    .unwrap_or_else(|| "unset".into())
}
impl Controller {
    fn runtime_number(
        &self,
        context: Context,
        field: RuntimeField,
        title: &'static str,
        hint: &'static str,
    ) -> Element<'_, Action> {
        let ticket = self.ticket();
        let editing = self.allowed(context)
            && self.flight.is_none()
            && !self.usage.pending()
            && !self.runtime.saving();
        let input = text_input("Blank = inherit", self.runtime.number(field))
            .padding(8)
            .size(12)
            .width(Length::Fill)
            .style(design::input_style)
            .on_input_maybe(editing.then_some(move |value| Action::Runtime {
                ticket,
                action: RuntimeAction::Number { field, value },
            }));
        let mut body = column![
            label(title, TEXT).size(12),
            input,
            label(hint, MUTED).size(11)
        ]
        .spacing(6);
        if let Some(snapshot) = self.runtime.snapshot {
            body = body.push(
                label(
                    format!(
                        "Last read effective: {} · inherited: {}",
                        numeric_hint(snapshot.values, field),
                        numeric_hint(snapshot.base_values, field)
                    ),
                    MUTED,
                )
                .size(11),
            );
        }
        body.into()
    }
    pub(super) fn runtime_view(&self, context: Context, images: bool) -> Column<'_, Action> {
        let ticket = self.ticket();
        let action = move |action| Action::Runtime { ticket, action };
        let editing = self.allowed(context)
            && self.flight.is_none()
            && !self.usage.pending()
            && !self.runtime.saving();
        let ready = self.allowed(context) && self.ready();
        let mut body = column![
            label(
                if images {
                    "Image-input limits · local draft"
                } else {
                    "Runtime settings · local draft"
                },
                TEXT
            )
            .size(15)
        ]
        .spacing(10);
        if !images {
            let transport = Transport(self.runtime.transport);
            let transport: Element<'_, Action> = if editing {
                pick_list(
                    [
                        Transport(None),
                        Transport(Some(CodexRuntimeTransport::Sse)),
                        Transport(Some(CodexRuntimeTransport::Websocket)),
                        Transport(Some(CodexRuntimeTransport::WebsocketCached)),
                        Transport(Some(CodexRuntimeTransport::Auto)),
                    ],
                    Some(transport),
                    move |value| action(RuntimeAction::Transport(value.0)),
                )
                .padding(8)
                .text_size(12)
                .width(Length::Fill)
                .style(design::picker_style)
                .menu_style(menu)
                .into()
            } else {
                container(label(transport.to_string(), MUTED).size(12))
                    .padding(8)
                    .into()
            };
            let cache = Cache(self.runtime.cache);
            let cache: Element<'_, Action> = if editing {
                pick_list(
                    [
                        Cache(None),
                        Cache(Some(CodexCacheRetention::None)),
                        Cache(Some(CodexCacheRetention::Short)),
                        Cache(Some(CodexCacheRetention::Long)),
                    ],
                    Some(cache),
                    move |value| action(RuntimeAction::Cache(value.0)),
                )
                .padding(8)
                .text_size(12)
                .width(Length::Fill)
                .style(design::picker_style)
                .menu_style(menu)
                .into()
            } else {
                container(label(cache.to_string(), MUTED).size(12))
                    .padding(8)
                    .into()
            };
            body = body.push(
                row![
                    column![label("Transport", TEXT).size(12), transport]
                        .spacing(6)
                        .width(Length::Fill),
                    column![label("Cache retention", TEXT).size(12), cache]
                        .spacing(6)
                        .width(Length::Fill),
                ]
                .spacing(12),
            );
            if let Some(snapshot) = self.runtime.snapshot {
                body = body.push(
                    label(
                        format!(
                            "Last read transport: {} · inherited: {} | cache: {} · inherited: {}",
                            Transport(snapshot.values.transport),
                            Transport(snapshot.base_values.transport),
                            Cache(snapshot.values.cache_retention),
                            Cache(snapshot.base_values.cache_retention)
                        ),
                        MUTED,
                    )
                    .size(11),
                );
            }
            body = body
                .push(self.runtime_number(
                    context,
                    RuntimeField::Timeout,
                    "Request timeout (ms)",
                    "Whole number: 0–9007199254740991.",
                ))
                .push(self.runtime_number(
                    context,
                    RuntimeField::WebsocketConnectTimeout,
                    "WebSocket connect timeout (ms)",
                    "Whole number: 0–9007199254740991.",
                ))
                .push(self.runtime_number(
                    context,
                    RuntimeField::StreamIdleTimeout,
                    "Stream idle timeout (ms)",
                    "Greater than 0, up to 2147483647; decimals allowed.",
                ));
        } else {
            for (field, title) in [
                (RuntimeField::ImagePixelBudget, "Image-input pixel budget"),
                (RuntimeField::ImageMaxBytes, "Per-image input limit (bytes)"),
                (
                    RuntimeField::MaxRequestImageBytes,
                    "Total request image-input limit (bytes)",
                ),
            ] {
                body = body.push(self.runtime_number(
                    context,
                    field,
                    title,
                    "Positive whole number, up to 9007199254740991.",
                ));
            }
        }
        body = body.push(label("Default / inherit and blank inputs remove the override. Save applies all runtime and image-input draft fields, not just this tab.", MUTED).size(12))
            .push(row![
                button("Save runtime").padding([8, 12]).style(design::primary_button)
                    .on_press_maybe((ready && self.runtime.can_save()).then_some(action(RuntimeAction::Save))),
                button("Refresh runtime").padding([8, 12]).style(design::button_style)
                    .on_press_maybe(ready.then_some(action(RuntimeAction::Refresh))),
                button("Reset to inherited defaults").padding([8, 12]).style(design::button_style)
                    .on_press_maybe(editing.then_some(action(RuntimeAction::Reset))),
            ].spacing(8));
        if let Some(snapshot) = self.runtime.snapshot {
            body = body.push(
                label(
                    format!(
                        "Last read revision {} · {} · {}",
                        snapshot.revision,
                        if snapshot.configured {
                            "configured"
                        } else {
                            "inherited configuration"
                        },
                        if snapshot.writable {
                            "writable"
                        } else {
                            "read-only"
                        }
                    ),
                    MUTED,
                )
                .size(11),
            );
        }
        if self.runtime.values().is_err() && !self.runtime.status().starts_with("Invalid") {
            body = body.push(label("Invalid draft: whole numbers only, except stream idle decimals. Blank inherits. Save is disabled.", DANGER).size(12));
        }
        body.push(
            label(
                self.runtime.status(),
                if self.runtime.attention() {
                    DANGER
                } else {
                    MUTED
                },
            )
            .size(12),
        )
    }
}
