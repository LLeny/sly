use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

use gpui::prelude::*;
use gpui::{AnyElement, AnyView, App, Context, Div, Entity, Render, div, px};
use workspace_model::{
    ComponentManifest, DockLocation, ManifestError, PanelComposition, PanelDescriptor,
    PanelGroupLayout, RefreshPolicy,
};

type RefreshCallback = dyn Fn(PanelRefresh, &mut App);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PanelRefresh {
    pub live: bool,
}

pub trait RefreshablePanel: Render {
    fn refresh(&mut self, request: PanelRefresh, cx: &mut Context<Self>);

    fn is_collapsed(&self) -> bool {
        false
    }
}

#[derive(Clone)]
pub struct PanelHandle {
    view: AnyView,
    refresh: Rc<RefreshCallback>,
}

impl PanelHandle {
    pub fn new<V>(entity: Entity<V>) -> Self
    where
        V: RefreshablePanel,
    {
        let refresh_entity = entity.clone();
        Self {
            view: entity.into(),
            refresh: Rc::new(move |request, cx| {
                refresh_entity.update(cx, |panel, cx| {
                    if !panel.is_collapsed() {
                        panel.refresh(request, cx);
                    }
                });
            }),
        }
    }

    pub fn view(&self) -> AnyView {
        self.view.clone()
    }

    fn refresh(&self, request: PanelRefresh, cx: &mut App) {
        (self.refresh)(request, cx);
    }
}

pub trait PanelFactory {
    fn create(&mut self, descriptor: &PanelDescriptor, cx: &mut App) -> PanelHandle;
}

struct PanelEntry {
    descriptor: PanelDescriptor,
    handle: PanelHandle,
}

pub struct PanelRegistry {
    manifest: ComponentManifest,
    entries: Vec<PanelEntry>,
    by_id: HashMap<Arc<str>, usize>,
}

impl PanelRegistry {
    pub fn new(
        manifest: ComponentManifest,
        factory: &mut impl PanelFactory,
        cx: &mut App,
    ) -> Result<Self, ManifestError> {
        manifest.validate()?;

        let mut entries = Vec::with_capacity(manifest.panels.len());
        let mut by_id = HashMap::with_capacity(manifest.panels.len());
        for descriptor in &manifest.panels {
            let ix = entries.len();
            by_id.insert(descriptor.id.clone(), ix);
            entries.push(PanelEntry {
                descriptor: descriptor.clone(),
                handle: factory.create(descriptor, cx),
            });
        }

        Ok(Self {
            manifest,
            entries,
            by_id,
        })
    }

    pub fn manifest(&self) -> &ComponentManifest {
        &self.manifest
    }

    pub fn view(&self, id: &str) -> Option<AnyView> {
        self.entry(id).map(|entry| entry.handle.view())
    }

    pub fn refresh_visible(
        &self,
        workspace: &str,
        request: PanelRefresh,
        live_refresh: bool,
        cx: &mut App,
    ) {
        for entry in &self.entries {
            if !entry.descriptor.visible_in(workspace) {
                continue;
            }

            if request.live
                && !live_refresh
                && entry.descriptor.refresh != RefreshPolicy::Continuous
            {
                continue;
            }

            entry.handle.refresh(request, cx);
        }
    }

    pub fn workspace_dock(&self, workspace: &str) -> WorkspaceDock {
        let compositions: HashMap<&str, &PanelComposition> = self
            .manifest
            .compositions
            .iter()
            .filter(|composition| composition.workspace.as_ref() == workspace)
            .map(|composition| (composition.host.as_ref(), composition))
            .collect();
        let companions: HashSet<&str> = compositions
            .values()
            .map(|composition| composition.companion.as_ref())
            .collect();

        let groups = |location| self.groups(workspace, location, &compositions, &companions);
        WorkspaceDock {
            left: groups(DockLocation::Left),
            center: groups(DockLocation::Center),
            right: groups(DockLocation::Right),
            bottom: groups(DockLocation::Bottom),
        }
    }

    fn entry(&self, id: &str) -> Option<&PanelEntry> {
        self.by_id.get(id).and_then(|ix| self.entries.get(*ix))
    }

    fn groups(
        &self,
        workspace: &str,
        location: DockLocation,
        compositions: &HashMap<&str, &PanelComposition>,
        companions: &HashSet<&str>,
    ) -> Vec<PanelGroup> {
        self.manifest
            .in_location(workspace, location)
            .filter(|descriptor| !companions.contains(descriptor.id.as_ref()))
            .filter_map(|descriptor| {
                let host = self.entry(&descriptor.id)?;
                let composition = compositions.get(descriptor.id.as_ref()).copied();
                Some(PanelGroup {
                    host: host.handle.view(),
                    companion: composition.and_then(|composition| {
                        self.entry(&composition.companion)
                            .map(|entry| entry.handle.view())
                    }),
                    layout: composition.map(|composition| composition.layout),
                })
            })
            .collect()
    }
}

struct PanelGroup {
    host: AnyView,
    companion: Option<AnyView>,
    layout: Option<PanelGroupLayout>,
}

impl PanelGroup {
    fn into_element(self) -> AnyElement {
        let Some(companion) = self.companion else {
            return self.host.into_any_element();
        };

        let host = panel_column().flex_1().child(self.host);
        let row = div().flex().gap_2().child(host);
        match self.layout.unwrap_or(PanelGroupLayout::EqualColumns) {
            PanelGroupLayout::EqualColumns => row
                .flex_1()
                .min_h(px(0.))
                .child(panel_column().flex_1().child(companion))
                .into_any_element(),
            PanelGroupLayout::CompanionRail => row
                .flex_1()
                .min_h(px(0.))
                .child(panel_column().w(px(300.)).flex_none().child(companion))
                .into_any_element(),
            PanelGroupLayout::CompactColumns => row
                .flex_none()
                .child(panel_column().flex_1().child(companion))
                .into_any_element(),
        }
    }
}

fn panel_column() -> Div {
    div().flex().flex_col().min_w(px(0.)).min_h(px(0.))
}

#[derive(IntoElement)]
pub struct WorkspaceDock {
    left: Vec<PanelGroup>,
    center: Vec<PanelGroup>,
    right: Vec<PanelGroup>,
    bottom: Vec<PanelGroup>,
}

impl RenderOnce for WorkspaceDock {
    fn render(self, _window: &mut gpui::Window, _cx: &mut App) -> impl IntoElement {
        let groups = |groups: Vec<PanelGroup>| groups.into_iter().map(PanelGroup::into_element);

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.))
            .gap_2()
            .p_2()
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(0.))
                    .gap_2()
                    .child(
                        panel_column()
                            .w(px(300.))
                            .flex_none()
                            .overflow_hidden()
                            .gap_2()
                            .children(groups(self.left)),
                    )
                    .child(
                        panel_column()
                            .flex_1()
                            .gap_2()
                            .children(groups(self.center)),
                    )
                    .child(
                        panel_column()
                            .w(px(300.))
                            .flex_none()
                            .gap_2()
                            .children(groups(self.right)),
                    ),
            )
            .children(groups(self.bottom))
    }
}
