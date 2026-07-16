use std::sync::Arc;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DockLocation {
    Left,
    Center,
    Right,
    Bottom,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RefreshPolicy {
    OnPause,
    Continuous,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PanelKind {
    Controls,
    Breakpoints,
    Cartridge,
    Registers { cpu: Arc<str> },
    Disassembly { cpu: Arc<str> },
    Memory,
    Mmio,
    Dma,
    Framebuffer,
    Palette,
    Ppu,
    Tileset,
    Tilemap,
    Sprites,
    Structured,
    Table,
    ChannelScopes,
    AudioScope,
    Log,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WorkspaceIcon {
    Cpu,
    Video,
    Sound,
}

#[derive(Clone, Debug)]
pub struct WorkspaceDescriptor {
    pub id: Arc<str>,
    pub title: Arc<str>,
    pub icon: WorkspaceIcon,
}

impl WorkspaceDescriptor {
    pub fn new(id: impl Into<Arc<str>>, title: impl Into<Arc<str>>, icon: WorkspaceIcon) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            icon,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PanelDescriptor {
    pub id: Arc<str>,
    pub title: Arc<str>,
    pub kind: PanelKind,
    pub location: DockLocation,
    pub refresh: RefreshPolicy,
    pub workspaces: Vec<Arc<str>>,
    pub selection_from: Option<Arc<str>>,
    pub companion: Option<Arc<str>>,
}

impl PanelDescriptor {
    pub fn new(
        id: impl Into<Arc<str>>,
        title: impl Into<Arc<str>>,
        kind: PanelKind,
        location: DockLocation,
        refresh: RefreshPolicy,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            kind,
            location,
            refresh,
            workspaces: Vec::new(),
            selection_from: None,
            companion: None,
        }
    }

    pub fn tag(mut self, workspace: impl Into<Arc<str>>) -> Self {
        self.workspaces.push(workspace.into());
        self
    }

    pub fn selection_from(mut self, table_id: impl Into<Arc<str>>) -> Self {
        self.selection_from = Some(table_id.into());
        self
    }

    pub fn companion(mut self, panel_id: impl Into<Arc<str>>) -> Self {
        self.companion = Some(panel_id.into());
        self
    }
}

#[derive(Clone, Debug)]
pub struct ComponentManifest {
    pub system: Arc<str>,
    pub workspaces: Vec<WorkspaceDescriptor>,
    pub panels: Vec<PanelDescriptor>,
}

impl ComponentManifest {
    pub fn in_workspace<'a>(
        &'a self,
        workspace: &'a str,
    ) -> impl Iterator<Item = &'a PanelDescriptor> {
        self.panels.iter().filter(move |p| {
            p.workspaces.is_empty() || p.workspaces.iter().any(|w| &**w == workspace)
        })
    }

    pub fn in_location<'a>(
        &'a self,
        workspace: &'a str,
        location: DockLocation,
    ) -> impl Iterator<Item = &'a PanelDescriptor> {
        self.in_workspace(workspace)
            .filter(move |p| p.location == location)
    }

    pub fn insert_after(&mut self, anchor: PanelKind, panel: PanelDescriptor) {
        let ix = self
            .panels
            .iter()
            .position(|p| p.kind == anchor)
            .map_or(self.panels.len(), |ix| ix + 1);
        self.panels.insert(ix, panel);
    }

    /// Tag the panel with the given id for a workspace, if not already tagged.
    pub fn tag(&mut self, id: &str, workspace: impl Into<Arc<str>>) {
        if let Some(panel) = self.panels.iter_mut().find(|p| &*p.id == id) {
            let workspace = workspace.into();
            if !panel.workspaces.contains(&workspace) {
                panel.workspaces.push(workspace);
            }
        }
    }

    /// Move the panel with the given id to `location`, re-inserting it right
    /// after the first panel of `anchor` kind (or at the end if absent).
    pub fn relocate_after(&mut self, id: &str, anchor: PanelKind, location: DockLocation) {
        let Some(pos) = self.panels.iter().position(|p| &*p.id == id) else {
            return;
        };

        let mut panel = self.panels.remove(pos);
        panel.location = location;
        self.insert_after(anchor, panel);
    }

    pub fn restrict(&mut self, kind: PanelKind, workspace: impl Into<Arc<str>>) {
        if let Some(panel) = self.panels.iter_mut().find(|p| p.kind == kind) {
            debug_assert!(
                panel.workspaces.is_empty(),
                "restrict() on already-tagged panel `{}`",
                panel.id,
            );
            panel.workspaces.push(workspace.into());
        }
    }

    pub fn insert_before(&mut self, anchor: PanelKind, panel: PanelDescriptor) {
        let ix = self
            .panels
            .iter()
            .position(|p| p.kind == anchor)
            .unwrap_or(self.panels.len());
        self.panels.insert(ix, panel);
    }

    pub fn validate(&self) {
        for panel in &self.panels {
            for tag in &panel.workspaces {
                debug_assert!(
                    self.workspaces.iter().any(|w| w.id == *tag),
                    "panel `{}` tagged with undeclared workspace `{tag}`",
                    panel.id,
                );
            }

            for link in [&panel.selection_from, &panel.companion]
                .into_iter()
                .flatten()
            {
                debug_assert!(
                    self.panels.iter().any(|p| p.id == *link),
                    "panel `{}` links to unknown panel `{link}`",
                    panel.id,
                );
            }
        }
    }
}
