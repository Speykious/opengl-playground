use glam::{Mat4, Vec2, Vec4, Vec4Swizzles};

#[derive(Clone)]
pub struct Camera {
    pub position: Vec2,
    pub rotation: f32,
    pub scale: Vec2,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            position: Vec2::ZERO,
            rotation: 0.0,
            scale: Vec2::ONE,
        }
    }
}

impl Camera {
    /// Gets the real size of the viewport
    pub fn real_size(&self, viewport: Vec2) -> Vec2 {
        Vec2 {
            x: viewport.x / self.scale.x,
            y: viewport.y / self.scale.y,
        }
    }

    /// Gets the center offset of the viewport
    pub fn center_offset(&self, viewport: Vec2) -> Vec2 {
        self.real_size(viewport) / 2.0
    }

    pub fn pointer_to_pos(&self, pointer: Vec2, viewport: Vec2) -> Vec2 {
        let origin = self.center_offset(viewport);
        let pos = self.position.extend(-(u16::MAX as f32 / 2.0));

        (
			Mat4::from_translation(-pos)
			* Mat4::from_translation(-origin.extend(0.0))
			* Mat4::from_rotation_z(-self.rotation)
			* Mat4::from_scale(1.0 / self.scale.extend(1.0))
            * Vec4::new(pointer.x, pointer.y, 0.0, 1.0)
		)
        .xy()
    }

    /// Gets the resulting matrix from the camera and viewport
    pub fn matrix(&self, viewport: Vec2) -> Mat4 {
        let real_size = self.real_size(viewport);

        // Faster to reuse real_size, so do that instead of calling get_center_offset
        let origin = real_size / 2.0;
        let pos = self.position.extend(-(u16::MAX as f32 / 2.0));

        // Return camera ortho matrix
        Mat4::orthographic_lh(0.0, real_size.x, real_size.y, 0.0, 0.0, u16::MAX as f32)
            * Mat4::from_translation(origin.extend(0.0))
            * Mat4::from_rotation_z(self.rotation)
            * Mat4::from_translation(pos)
    }
}
