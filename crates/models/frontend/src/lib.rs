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
    Log,
}

#[derive(Clone, Debug)]
pub struct PanelDescriptor {
    pub id: Arc<str>,
    pub title: Arc<str>,
    pub kind: PanelKind,
    pub location: DockLocation,
    pub refresh: RefreshPolicy,
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
        }
    }
}

#[derive(Clone, Debug)]
pub struct ComponentManifest {
    pub system: Arc<str>,
    pub panels: Vec<PanelDescriptor>,
}

impl ComponentManifest {
    pub fn in_location(&self, location: DockLocation) -> impl Iterator<Item = &PanelDescriptor> {
        self.panels.iter().filter(move |p| p.location == location)
    }
}
