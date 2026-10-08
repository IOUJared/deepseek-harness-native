//! Feature-only PUBLIC production App/operation rig. Never starts a worker or
//! subscribes to App runtime services; the example only drives owned native UI Tasks.
use super::*;
use serde_json::{Value, json};

#[derive(Default)]
struct Span {
    calls: std::cell::Cell<u64>,
    total_ns: std::cell::Cell<u64>,
    maximum_ns: std::cell::Cell<u64>,
}
impl Span {
    fn record(&self, elapsed: std::time::Duration) {
        let ns = u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX);
        self.calls.set(self.calls.get().saturating_add(1));
        self.total_ns.set(self.total_ns.get().saturating_add(ns));
        self.maximum_ns.set(self.maximum_ns.get().max(ns));
    }
    fn value(&self) -> Value {
        json!({"calls":self.calls.get(),"totalNs":self.total_ns.get(),"maximumNs":self.maximum_ns.get()})
    }
}
#[derive(Default)]
struct Metrics {
    boot: Span,
    view: Span,
    update: Span,
    receive: Span,
    diagnostic: Span,
    seed: Span,
}

pub struct Rig {
    app: App,
    commands: tokio::sync::mpsc::Receiver<Command>,
    command_count: usize,
    decision: Option<crate::interactions::Key>,
    seeded: bool,
    initial_rows: usize,
    expected_anchor: u64,
    metrics: Metrics,
}
fn record(seq: u64) -> Value {
    // A bounded PUBLIC word line near the wide-lane limit exercises native rewrap.
    let text = format!("PUBLIC durable row {seq}: {}", "wide ".repeat(9));
    json!({"type":"event","event":{"type":"user/message","seq":seq,"time":1000+seq,"surfaceOp":"append","data":{"content":[{"type":"text","text":text}]}}})
}
impl Rig {
    pub fn new() -> Self {
        Self::with_rows(80)
    }
    pub fn with_rows(rows: usize) -> Self {
        assert!(
            (80..=4084).contains(&rows),
            "PUBLIC rows plus twelve older rows must fit retention"
        );
        let started = std::time::Instant::now();
        let options = crate::config::Options {
            backend: dsh_native_core::RustBackendOptions {
                runtime: "/PUBLIC-scroll-fixture/NEVER-START/runtime".into(),
                expected_version: "0.2.1-alpha.1".into(),
                native_home: "/PUBLIC-scroll-fixture/NEVER-START/native-home".into(),
                user_home: "/PUBLIC-scroll-fixture/NEVER-START/user-home".into(),
                working_directory: "/PUBLIC-scroll-fixture/workspace".into(),
                absolute_node_path: None,
                startup_timeout: std::time::Duration::from_secs(1),
                stop_policy: Default::default(),
            },
            scale: 1.0,
            smoke: None,
        };
        let (mut app, commands) =
            super::layout_fixture::build(options, "conversation", iced::Size::new(1040.0, 800.0));
        let end = 100 + rows as u64;
        let records: Vec<_> = (100..end).map(record).collect();
        let snapshot=serde_json::from_value(json!({"type":"snapshot","records":records,"cursor":end-1,"hasMore":true,"header":{"version":4,"id":"PUBLIC-layout-conversation","createdAt":1000,"isSeeded":false},"projections":{"asOfSeq":end-1,"values":{}},"assistantStream":{"revision":0}})).unwrap();
        app.transcript = Transcript::new(app.generation);
        let metrics = Metrics::default();
        let received = std::time::Instant::now();
        drop(app.receive(Event::Selected {
            generation: app.generation,
            frame: snapshot,
        }));
        metrics.receive.record(received.elapsed());
        app.follow_bottom = false;
        // The initial real widget starts at zero. Do not invent an already-read
        // viewport before the owned native window exists and positioning executes.
        app.offset = 0.0;
        assert!(app.transcript_active() && app.transcript.safe_to_send());
        metrics.boot.record(started.elapsed());
        Self {
            app,
            commands,
            command_count: 0,
            decision: None,
            seeded: false,
            initial_rows: rows,
            expected_anchor: if rows == 80 {
                112
            } else {
                100 + rows as u64 / 2
            },
            metrics,
        }
    }
    pub fn view(&self) -> Element<'_, Message> {
        let started = std::time::Instant::now();
        let view = self.app.view();
        self.metrics.view.record(started.elapsed());
        view
    }
    pub fn update(&mut self, message: Message) -> Task<Message> {
        let started = std::time::Instant::now();
        let task = self.app.update(message);
        self.metrics.update.record(started.elapsed());
        task
    }
    fn receive(&mut self, event: Event) -> Task<Message> {
        let started = std::time::Instant::now();
        let task = self.app.receive(event);
        self.metrics.receive.record(started.elapsed());
        task
    }
    pub fn frame(&self) -> iced::Size {
        self.app.layout_size()
    }
    pub fn expected_anchor_key(&self) -> u64 {
        self.expected_anchor
    }
    /// Establish one scripted PUBLIC reader position after the real window opens;
    /// this is not restoring saved production startup state.
    pub fn position(&mut self) -> Task<Message> {
        assert!(!self.seeded, "PUBLIC initial reader position is one-use");
        let started = std::time::Instant::now();
        self.seeded = true;
        self.app.follow_bottom = false;
        let (keys, heights, _) = self.app.scroll_projection();
        let index = keys
            .iter()
            .position(|k| *k == self.expected_anchor)
            .unwrap();
        self.app.offset = heights[..index].iter().sum::<f32>() + 12.0;
        self.app.invalidate_scroll();
        let task = self.app.position_transcript();
        self.metrics.seed.record(started.elapsed());
        task
    }
    pub fn prepend(&mut self) -> Task<Message> {
        let records: Vec<_> = (88..100).map(record).collect();
        let page = serde_json::from_value(json!({"records":records,"hasMore":false})).unwrap();
        self.receive(Event::Page {
            generation: self.app.generation,
            result: Ok(page),
        })
    }
    pub fn open_decision(&mut self) -> Task<Message> {
        assert!(self.decision.is_none());
        let key = crate::interactions::Key {
            epoch: self.app.transport_epoch,
            event_id: RemoteEventId::new("PUBLIC-scroll-dialog").unwrap(),
            serial: 1,
        };
        let arrival=self.receive(Event::Root{frame:RemoteEventFrame::Waterfall{event:WaterfallKind::Approval,event_id:key.event_id.clone(),agent_id:AgentId::new("PUBLIC-scroll-agent").unwrap(),request:json!({"toolName":"PUBLIC no-execution tool","callId":"PUBLIC-scroll-call","reason":"PUBLIC native remount qualification"})},decision_key:Some(key.clone())});
        self.decision = Some(key.clone());
        Task::batch([
            arrival,
            self.update(Message::Interaction(crate::interactions::Action::Open(key))),
        ])
    }
    pub fn cancel_decision(&mut self) -> Task<Message> {
        let key = self.decision.take().expect("owned PUBLIC decision");
        self.receive(Event::Root {
            frame: RemoteEventFrame::Cancel {
                event_id: key.event_id,
            },
            decision_key: None,
        })
    }
    pub fn id(&self) -> String {
        self.app.scroll_id()
    }
    pub fn current_applied(&self, message: &Message) -> bool {
        matches!(message,Message::ScrollApplied{generation,revision,feedback,..} if *generation==self.app.generation && revision.is_some() && *revision==self.app.scroll_revision && feedback.is_some() && *feedback==self.app.scroll_feedback)
    }
    pub fn drain_commands(&mut self) -> usize {
        while self.commands.try_recv().is_ok() {
            self.command_count += 1;
        }
        self.command_count
    }
    /// Cheap PUBLIC scalar metadata: no paragraph measurement or projection allocation.
    pub fn scalar_model(&self) -> Value {
        json!({"generation":self.app.generation,"revision":self.app.scroll_revision,"feedback":self.app.scroll_feedback,"offset":self.app.offset,"viewport":self.app.viewport,"followBottom":self.app.follow_bottom,"active":self.app.transcript_active(),"commandCount":self.command_count,"initialRows":self.initial_rows,"expectedAnchorKey":self.expected_anchor,"keysCount":self.app.transcript.rows().len(),"laneWidth":(self.app.layout_size().width-self.app.sidebar_motion.value()-40.0).min(design::CHAT),"windowWidth":self.frame().width,"windowHeight":self.frame().height,"frameMeaning":"controlled application layout frame; native negotiated window size is independently observed by the example"})
    }
    pub fn metrics(&self) -> Value {
        let (entries, shapes, shape_ns, hits) = self.app.chat_geometry.observations();
        json!({"scope":"feature-only synchronous wall-clock spans, not thread CPU or compositor presentation; nested boot/receive and diagnostic/cache work must not be added or subtracted to invent product latency","boot":self.metrics.boot.value(),"productionView":self.metrics.view.value(),"productionUpdate":self.metrics.update.value(),"productionReceive":self.metrics.receive.value(),"diagnosticModel":self.metrics.diagnostic.value(),"scriptedInitialPosition":self.metrics.seed.value(),"cachedGeometryEntries":entries,"nativeParagraphShapes":shapes,"nativeParagraphShapeNs":shape_ns,"geometryCacheHits":hits,"initialRows":self.initial_rows,"expectedAnchorKey":self.expected_anchor})
    }
    pub fn model(&self) -> Value {
        let started = std::time::Instant::now();
        let (keys, heights, trailing) = self.app.scroll_projection();
        let mut top = 0.0;
        let mut anchor = None;
        for (&key, &height) in keys.iter().zip(&heights) {
            if self.app.offset >= top && self.app.offset < top + height {
                anchor = Some((key, self.app.offset - top, height));
                break;
            }
            top += height;
        }
        let mut model = self.scalar_model();
        model["anchorKey"] = json!(anchor.map(|v| v.0));
        model["anchorIntra"] = json!(anchor.map(|v| v.1));
        model["anchorHeight"] = json!(anchor.map(|v| v.2));
        model["keysCount"] = json!(keys.len());
        model["totalHeight"] = json!(heights.iter().sum::<f32>() + trailing);
        self.metrics.diagnostic.record(started.elapsed());
        model
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scalar_observation_and_frame_do_not_shape_or_prewarm_production_view() {
        let rig = Rig::new();
        let before = rig.metrics();
        for _ in 0..8 {
            assert_eq!(rig.frame().width, 1040.0);
            assert_eq!(rig.scalar_model()["expectedAnchorKey"], 112);
        }
        assert_eq!(
            rig.metrics()["nativeParagraphShapes"],
            before["nativeParagraphShapes"]
        );
        assert_eq!(rig.metrics()["diagnosticModel"]["calls"], 0);
        drop(rig.view());
        let after = rig.metrics();
        // Initial Selected receive can already measure the durable projection;
        // passive inspection must not be credited with (or manufacture) cold view work.
        assert!(
            after["geometryCacheHits"].as_u64().unwrap()
                > before["geometryCacheHits"].as_u64().unwrap()
        );
        assert_eq!(after["productionView"]["calls"], 1);
        drop(rig.view());
        assert_eq!(
            rig.metrics()["nativeParagraphShapes"],
            after["nativeParagraphShapes"]
        );
    }
    #[test]
    fn full_retention_mid_history_reflow_and_prepend_preserve_public_identity() {
        let mut rig = Rig::with_rows(4084);
        assert_eq!(rig.expected_anchor_key(), 2142);
        drop(rig.position());
        let before = rig.model();
        assert_eq!(before["anchorKey"], 2142);
        assert_eq!(before["keysCount"], 4084);
        drop(rig.update(Message::Resized(iced::Size::new(650.0, 800.0))));
        let narrow = rig.model();
        assert_eq!(narrow["anchorKey"], 2142);
        assert_eq!(narrow["anchorIntra"], 12.0);
        assert_ne!(before["offset"], narrow["offset"]);
        drop(rig.prepend());
        assert_eq!(rig.model()["keysCount"], 4096);
        assert_eq!(rig.model()["anchorKey"], 2142);
        assert_eq!(rig.model()["anchorIntra"], 12.0);
        assert!(rig.app.transcript.safe_to_send());
        assert_eq!(rig.drain_commands(), 0);
        assert!(rig.metrics()["cachedGeometryEntries"].as_u64().unwrap() <= 4097);
    }
    #[test]
    #[should_panic(expected = "must fit retention")]
    fn oversized_public_workload_is_rejected_before_snapshot_allocation() {
        drop(Rig::with_rows(4085));
    }
    #[test]
    fn native_rewrap_changes_actual_prefix_not_only_widget_identity() {
        let mut rig = Rig::new();
        drop(rig.position());
        let before = rig.model();
        drop(rig.update(Message::Resized(iced::Size::new(650.0, 800.0))));
        let after = rig.model();
        assert_eq!(after["anchorKey"], 112);
        assert_eq!(after["anchorIntra"], 12.0);
        assert_ne!(
            before["offset"], after["offset"],
            "PUBLIC text must actually rewrap at the target lane"
        );
        assert_eq!(rig.drain_commands(), 0);
    }
    #[test]
    fn rig_actions_are_public_model_only_and_queue_empty() {
        let mut rig = Rig::new();
        assert_eq!(rig.model()["anchorKey"], 100);
        drop(rig.position());
        assert_eq!(rig.model()["anchorKey"], 112);
        assert_eq!(rig.drain_commands(), 0);
        drop(rig.prepend());
        assert_eq!(rig.model()["keysCount"], 92);
        assert_eq!(rig.model()["anchorKey"], 112);
        drop(rig.open_decision());
        assert_eq!(rig.model()["active"], false);
        drop(rig.cancel_decision());
        assert_eq!(rig.model()["active"], true);
        assert_eq!(rig.model()["anchorKey"], 112);
        assert_eq!(rig.drain_commands(), 0);
    }
}
