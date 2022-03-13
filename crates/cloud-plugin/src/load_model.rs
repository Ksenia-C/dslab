pub trait LoadModel {
    fn get_resource_load(&self, time: f64, time_from_start: f64) -> f64;
}

pub struct ConstLoadModel {
    load: f64,
}

impl ConstLoadModel {
    pub fn new(load: f64) -> Self {
        Self { load }
    }
}

impl LoadModel for ConstLoadModel {
    fn get_resource_load(&self, _time: f64, _time_from_start: f64) -> f64 {
        self.load
    }
}
