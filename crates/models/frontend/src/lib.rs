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
    Palette,
    Ppu,
    Tileset,
    Log,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WorkspaceIcon {
    Cpu,
    Video,
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
        }
    }

    pub fn tag(mut self, workspace: impl Into<Arc<str>>) -> Self {
        self.workspaces.push(workspace.into());
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
        }
    }
}
