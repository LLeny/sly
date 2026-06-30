use gpui::SharedString;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct CpuId(SharedString);

impl CpuId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<T: Into<SharedString>> From<T> for CpuId {
    fn from(value: T) -> Self {
        Self(value.into())
    }
}

impl From<&CpuId> for SharedString {
    fn from(id: &CpuId) -> Self {
        id.0.clone()
    }
}

#[derive(Clone)]
pub struct CpuDescriptor {
    pub id: CpuId,
    pub title: SharedString,
    pub space: SharedString,
}

impl CpuDescriptor {
    pub fn new(
        id: impl Into<CpuId>,
        title: impl Into<SharedString>,
        space: impl Into<SharedString>,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            space: space.into(),
        }
    }
}
