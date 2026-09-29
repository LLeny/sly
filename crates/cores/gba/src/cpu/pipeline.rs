#[derive(Default)]
pub struct Pipeline {
    pub(crate) stages: [Option<(u32, u32)>; 3],
}

impl Pipeline {
    pub fn pending(&self) -> [Option<(u32, u32)>; 2] {
        [self.stages[1], self.stages[2]]
    }

    pub fn advance(&mut self, address: u32, instruction: u32) -> Option<(u32, u32)> {
        self.stages[0] = self.stages[1];
        self.stages[1] = self.stages[2];
        self.stages[2] = Some((address, instruction));
        self.stages[0]
    }

    pub fn flush(&mut self) {
        self.stages = [None; 3];
    }

    pub fn is_empty(&self) -> bool {
        self.stages.iter().all(Option::is_none)
    }
}
