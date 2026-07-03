pub trait Core {
    fn reset(&mut self);
    fn step(&mut self);
    fn run_frame(&mut self);
}
