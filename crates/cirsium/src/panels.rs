use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

use gpui::prelude::*;
use gpui::{AnyElement, AnyView, App, Context, Div, Entity, Render, div, px, svg};
use workspace_model::{
    ComponentManifest, DockLocation, ManifestError, PanelComposition, PanelDescriptor,
    PanelGroupLayout, RefreshPolicy,
};

use crate::assets::icon;
use crate::components::HEADER_HEIGHT;
use crate::theme::Theme;

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

    pub fn descriptor(&self, id: &str) -> Option<&PanelDescriptor> {
        self.entry(id).map(|entry| &entry.descriptor)
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

    pub fn workspace_dock(
        &self,
        workspace: &str,
        visible: &HashSet<String>,
        is_collapsed: impl Fn(&str) -> bool,
    ) -> WorkspaceDock {
        let compositions: HashMap<&str, &PanelComposition> = self
            .manifest
            .compositions
            .iter()
            .filter(|composition| composition.workspace.as_ref() == workspace)
            .filter(|composition| {
                visible.contains(composition.host.as_ref())
                    && visible.contains(composition.companion.as_ref())
            })
            .map(|composition| (composition.host.as_ref(), composition))
            .collect();
        let companions: HashSet<&str> = compositions
            .values()
            .map(|composition| composition.companion.as_ref())
            .collect();

        let groups = |location| {
            self.groups(
                workspace,
                location,
                visible,
                &compositions,
                &companions,
                &is_collapsed,
            )
        };
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

    fn groups<F: Fn(&str) -> bool>(
        &self,
        workspace: &str,
        location: DockLocation,
        visible: &HashSet<String>,
        compositions: &HashMap<&str, &PanelComposition>,
        companions: &HashSet<&str>,
        is_collapsed: &F,
    ) -> Vec<PanelGroup> {
        let mut groups: Vec<PanelGroup> = self
            .manifest
            .in_location(workspace, location)
            .filter(|descriptor| visible.contains(descriptor.id.as_ref()))
            .filter(|descriptor| !companions.contains(descriptor.id.as_ref()))
            .filter_map(|descriptor| {
                let host = self.entry(&descriptor.id)?;
                let composition = compositions.get(descriptor.id.as_ref()).copied();
                Some(PanelGroup {
                    host: host.handle.view(),
                    host_collapsed: is_collapsed(descriptor.id.as_ref()),
                    companion: composition.and_then(|composition| {
                        self.entry(&composition.companion)
                            .map(|entry| entry.handle.view())
                    }),
                    layout: composition.map(|composition| composition.layout),
                })
            })
            .collect();

        groups.sort_by_key(|group| group.host_collapsed);
        groups
    }
}

struct PanelGroup {
    host: AnyView,
    host_collapsed: bool,
    companion: Option<AnyView>,
    layout: Option<PanelGroupLayout>,
}

impl PanelGroup {
    fn into_element(self, t: &Theme) -> AnyElement {
        let Some(companion) = self.companion else {
            return self.host.into_any_element();
        };

        let grow = !self.host_collapsed;
        let host = panel_column().flex_1().child(self.host);
        let row = div().flex().gap_2().child(host);
        let sized = |row: Div| {
            if grow {
                row.flex_1().min_h(px(0.))
            } else {
                row.flex_none()
            }
        };

        match self.layout.unwrap_or(PanelGroupLayout::EqualColumns) {
            PanelGroupLayout::EqualColumns => {
                sized(row.child(link_seam_left(t, panel_column().flex_1().child(companion))))
                    .into_any_element()
            }
            PanelGroupLayout::CompanionRail => sized(row.child(link_seam_left(
                t,
                panel_column().w(px(300.)).flex_none().child(companion),
            )))
            .into_any_element(),
            PanelGroupLayout::CompactColumns => row
                .flex_none()
                .child(link_seam_left(t, panel_column().flex_1().child(companion)))
                .into_any_element(),
        }
    }
}

fn panel_column() -> Div {
    div().flex().flex_col().min_w(px(0.)).min_h(px(0.))
}

fn link_chip(t: &Theme) -> Div {
    div()
        .flex()
        .items_center()
        .justify_center()
        .size(px(14.))
        .rounded_full()
        .bg(t.bg_root)
        .child(
            svg()
                .path(icon::LINK)
                .size(px(10.))
                .text_color(t.text_muted),
        )
}

fn link_seam_left(t: &Theme, column: Div) -> Div {
    column.relative().child(
        link_chip(t)
            .absolute()
            .left(px(-11.))
            .top(px(HEADER_HEIGHT / 2. - 7.)),
    )
}

#[derive(IntoElement)]
pub struct WorkspaceDock {
    left: Vec<PanelGroup>,
    center: Vec<PanelGroup>,
    right: Vec<PanelGroup>,
    bottom: Vec<PanelGroup>,
}

impl RenderOnce for WorkspaceDock {
    fn render(self, _window: &mut gpui::Window, cx: &mut App) -> impl IntoElement {
        let theme = Theme::global(cx);
        let groups = move |groups: Vec<PanelGroup>| {
            let theme = theme.clone();
            groups
                .into_iter()
                .map(move |group| group.into_element(&theme))
        };

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
