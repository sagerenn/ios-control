pub struct CoordinateMapper {
    device_size: (u32, u32),
    pointer_estimate: (f32, f32),
    uncertainty_radius: f32,
}

impl CoordinateMapper {
    pub fn new(
        device_size: (u32, u32),
        pointer_estimate: (f32, f32),
        uncertainty_radius: f32,
    ) -> Self {
        Self {
            device_size,
            pointer_estimate,
            uncertainty_radius,
        }
    }

    pub fn can_confidently_hit(&self, region: (u32, u32, u32, u32)) -> bool {
        let (_, _, width, height) = region;
        (width as f32) > self.uncertainty_radius * 2.0
            && (height as f32) > self.uncertainty_radius * 2.0
    }

    pub fn device_size(&self) -> (u32, u32) {
        self.device_size
    }
}
