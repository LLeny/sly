use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use cirsium::{PanelFactory, PanelHandle, PanelRefresh, RefreshablePanel};
use gpui::prelude::*;
use gpui::{AnyElement, App, Context, Render, SharedString, Window, div};
use workspace_model::PanelDescriptor;

#[derive(Clone, Default)]
pub(super) struct PanelFrames(Rc<RefCell<HashMap<String, AnyElement>>>);

impl PanelFrames {
    pub(super) fn replace(&self, panels: HashMap<String, AnyElement>) {
        *self.0.borrow_mut() = panels;
    }

    fn take(&self, id: &str) -> Option<AnyElement> {
        self.0.borrow_mut().remove(id)
    }
}

pub(super) struct DebuggerPanelFactory {
    frames: PanelFrames,
}

impl DebuggerPanelFactory {
    pub(super) fn new(frames: PanelFrames) -> Self {
        Self { frames }
    }
}

impl PanelFactory for DebuggerPanelFactory {
    fn create(&mut self, descriptor: &PanelDescriptor, cx: &mut App) -> PanelHandle {
        PanelHandle::new(cx.new(|_| RegisteredPanel {
            id: SharedString::from(descriptor.id.clone()),
            frames: self.frames.clone(),
        }))
    }
}

struct RegisteredPanel {
    id: SharedString,
    frames: PanelFrames,
}

impl RefreshablePanel for RegisteredPanel {
    fn refresh(&mut self, _request: PanelRefresh, cx: &mut Context<Self>) {
        cx.notify();
    }
}

impl Render for RegisteredPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.frames
            .take(&self.id)
            .unwrap_or_else(|| div().into_any_element())
    }
}
