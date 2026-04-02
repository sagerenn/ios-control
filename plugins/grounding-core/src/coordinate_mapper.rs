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
        let (x, y, width, height) = region;
        let (pointer_x, pointer_y) = self.pointer_estimate;
        let within_region = pointer_x >= x as f32
            && pointer_x <= (x + width) as f32
            && pointer_y >= y as f32
            && pointer_y <= (y + height) as f32;

        (width as f32) > self.uncertainty_radius * 2.0
            && (height as f32) > self.uncertainty_radius * 2.0
            && within_region
    }

    pub fn device_size(&self) -> (u32, u32) {
        self.device_size
    }
}
