//! Restore a local transcript offset against the rebuilt widget's actual geometry.
//! No retained text, model mutation, delayed frame task or absent-widget acknowledgment.
use iced::advanced::widget::{Id, Operation, operation};
use iced::{Rectangle, Task, Vector};
use std::sync::Mutex;

pub(crate) fn restore<M: Send + 'static>(
    target_id: String,
    target: f32,
    on_applied: impl FnOnce(f32, f32) -> M + Send + 'static,
) -> Task<M> {
    if !target.is_finite() {
        return Task::none();
    }
    iced::advanced::widget::operate(Restore::new(target_id, target, on_applied))
}

struct Restore<F> {
    id: Id,
    target: f32,
    applied: Option<(f32, f32)>,
    // Operation::finish takes &self; consume this one-use callback without cloning it.
    on_applied: Mutex<Option<F>>,
}

impl<F> Restore<F> {
    fn new(target_id: String, target: f32, on_applied: F) -> Self {
        Self {
            id: Id::from(target_id),
            target,
            applied: None,
            on_applied: Mutex::new(Some(on_applied)),
        }
    }
}

impl<M, F: FnOnce(f32, f32) -> M + Send> Operation<M> for Restore<F> {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<M>)) {
        operate(self);
    }

    fn scrollable(
        &mut self,
        id: Option<&Id>,
        bounds: Rectangle,
        content_bounds: Rectangle,
        _translation: Vector,
        state: &mut dyn operation::Scrollable,
    ) {
        if id != Some(&self.id)
            || self.applied.is_some()
            || !self.target.is_finite()
            || !valid_rectangle(bounds)
            || bounds.width <= 0.0
            || bounds.height <= 0.0
            || !valid_rectangle(content_bounds)
        {
            return;
        }
        let maximum = (content_bounds.height - bounds.height).max(0.0);
        let actual = self.target.clamp(0.0, maximum);
        state.scroll_to(operation::scrollable::AbsoluteOffset {
            x: None,
            y: Some(actual),
        });
        self.applied = Some((actual, bounds.height));
    }

    fn finish(&self) -> operation::Outcome<M> {
        let Some((actual, height)) = self.applied else {
            return operation::Outcome::None;
        };
        let callback = self
            .on_applied
            .lock()
            .ok()
            .and_then(|mut callback| callback.take());
        match callback {
            Some(callback) => operation::Outcome::Some(callback(actual, height)),
            None => operation::Outcome::None,
        }
    }
}

fn valid_rectangle(rectangle: Rectangle) -> bool {
    rectangle.x.is_finite()
        && rectangle.y.is_finite()
        && rectangle.width.is_finite()
        && rectangle.height.is_finite()
        && rectangle.width >= 0.0
        && rectangle.height >= 0.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use operation::scrollable::{AbsoluteOffset, RelativeOffset};

    #[derive(Default)]
    struct FakeScrollable {
        offset: Option<AbsoluteOffset<Option<f32>>>,
        calls: usize,
    }
    impl operation::Scrollable for FakeScrollable {
        fn snap_to(&mut self, _: RelativeOffset<Option<f32>>) {
            panic!("restore must not snap");
        }
        fn scroll_to(&mut self, offset: AbsoluteOffset<Option<f32>>) {
            self.offset = Some(offset);
            self.calls += 1;
        }
        fn scroll_by(&mut self, _: AbsoluteOffset, _: Rectangle, _: Rectangle) {
            panic!("restore must not scroll by a stale delta");
        }
    }
    fn rectangle(height: f32) -> Rectangle {
        Rectangle {
            x: 10.0,
            y: 30.0,
            width: 100.0,
            height,
        }
    }

    #[test]
    fn actual_layout_clamps_and_preserves_horizontal_state() {
        for (target, viewport, content, expected) in [
            (900.0, 200.0, 500.0, 300.0),
            (-12.0, 200.0, 500.0, 0.0),
            (120.0, 200.0, 500.0, 120.0),
            (900.0, 700.0, 500.0, 0.0),
            (10.0, 200.0, 200.0, 0.0),
            (10.0, 200.0, 0.0, 0.0),
        ] {
            let mut operation = Restore::new("native-transcript".into(), target, |y, h| (y, h));
            let mut state = FakeScrollable::default();
            let id = Id::new("native-transcript");
            operation.scrollable(
                Some(&id),
                rectangle(viewport),
                rectangle(content),
                Vector::ZERO,
                &mut state,
            );
            assert_eq!(state.calls, 1);
            assert_eq!(state.offset.unwrap().x, None);
            assert_eq!(state.offset.unwrap().y, Some(expected));
            assert!(
                matches!(operation.finish(), operation::Outcome::Some((y,h)) if y == expected && h == viewport)
            );
            assert!(matches!(operation.finish(), operation::Outcome::None));
        }
    }

    #[test]
    fn absent_or_foreign_ids_never_change_state_or_acknowledge() {
        let mut operation = Restore::new("native-transcript".into(), 20.0, |y, h| (y, h));
        let mut state = FakeScrollable::default();
        let foreign = Id::new("other-scrollable");
        for id in [None, Some(&foreign)] {
            operation.scrollable(
                id,
                rectangle(100.0),
                rectangle(500.0),
                Vector::ZERO,
                &mut state,
            );
        }
        assert_eq!(state.calls, 0);
        assert!(matches!(operation.finish(), operation::Outcome::None));
    }

    #[test]
    fn invalid_targets_and_geometry_never_change_state_or_acknowledge() {
        let id = Id::new("native-transcript");
        for target in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut operation = Restore::new("native-transcript".into(), target, |y, h| (y, h));
            let mut state = FakeScrollable::default();
            operation.scrollable(
                Some(&id),
                rectangle(100.0),
                rectangle(500.0),
                Vector::ZERO,
                &mut state,
            );
            assert_eq!(state.calls, 0);
            assert!(matches!(operation.finish(), operation::Outcome::None));
        }
        for (bounds, content) in [
            (rectangle(0.0), rectangle(500.0)),
            (rectangle(-1.0), rectangle(500.0)),
            (rectangle(f32::NAN), rectangle(500.0)),
            (rectangle(100.0), rectangle(f32::INFINITY)),
            (rectangle(100.0), rectangle(-1.0)),
            (
                Rectangle {
                    x: f32::NAN,
                    ..rectangle(100.0)
                },
                rectangle(500.0),
            ),
            (
                rectangle(100.0),
                Rectangle {
                    width: f32::NAN,
                    ..rectangle(500.0)
                },
            ),
        ] {
            let mut operation = Restore::new("native-transcript".into(), 20.0, |y, h| (y, h));
            let mut state = FakeScrollable::default();
            operation.scrollable(Some(&id), bounds, content, Vector::ZERO, &mut state);
            assert_eq!(state.calls, 0);
            assert!(matches!(operation.finish(), operation::Outcome::None));
        }
    }

    #[test]
    fn first_valid_match_and_callback_are_one_use() {
        let id = Id::new("native-transcript");
        let mut operation = Restore::new("native-transcript".into(), 80.0, |y, h| (y, h));
        let mut state = FakeScrollable::default();
        operation.scrollable(
            Some(&id),
            rectangle(100.0),
            rectangle(500.0),
            Vector::ZERO,
            &mut state,
        );
        operation.scrollable(
            Some(&id),
            rectangle(200.0),
            rectangle(500.0),
            Vector::ZERO,
            &mut state,
        );
        assert_eq!(state.calls, 1);
        assert!(matches!(
            operation.finish(),
            operation::Outcome::Some((80.0, 100.0))
        ));
        assert!(matches!(operation.finish(), operation::Outcome::None));
    }
}
