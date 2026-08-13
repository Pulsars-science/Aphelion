//! Cameras that survive astronomical distances.
//!
//! Rendering a solar system breaks two assumptions that ordinary 3D engines
//! rest on, and both are dealt with here.
//!
//! **The world does not fit in an `f32`.** Neptune is 4.5e12 m from the Sun; an
//! `f32` has about seven significant digits, so a position in metres would be
//! quantised to tens of thousands of kilometres — larger than the planet. Two
//! things fix it: positions are kept in `f64` on the CPU and only converted at
//! the last moment, and they are converted *relative to the camera*, so the
//! numbers handed to the GPU are small wherever the viewer happens to be. The
//! unit sent across is the astronomical unit ([`RENDER_UNIT`]) rather than the
//! metre, which buys back a further eleven orders of magnitude of headroom.
//!
//! **The depth range does not fit in a depth buffer.** Seeing a moon 1000 km
//! away and a planet 30 AU away in the same frame is a near:far ratio of 1e12.
//! The answer is a *reverse-Z* projection with an infinite far plane: depth 1
//! at the near plane falling towards 0 at infinity. Because floating point
//! packs its precision near zero, and reverse-Z puts the far distances there,
//! the result is nearly uniform relative precision at every scale — and no far
//! plane to clip against.

use aphelion_core::constants::AU;
use glam::{DVec3, Mat4, Vec3};

/// The world unit sent to the GPU, in metres. One astronomical unit.
///
/// Scaling to AU before the `f64` → `f32` conversion is what keeps a planet's
/// surface smooth when the camera is 30 AU out.
pub const RENDER_UNIT: f64 = AU;

/// Converts a world position, in metres, to camera-relative render units.
#[inline]
pub fn to_render_space(position: DVec3, camera_position: DVec3) -> Vec3 {
    ((position - camera_position) / RENDER_UNIT).as_vec3()
}

/// Converts a length, in metres, to render units.
#[inline]
pub fn scale_to_render(metres: f64) -> f32 {
    (metres / RENDER_UNIT) as f32
}

/// A view onto the scene.
#[derive(Debug, Clone, Copy)]
pub struct Camera {
    /// Eye position, in metres, in the simulation frame.
    pub position: DVec3,
    /// Point the camera looks at, in metres.
    pub target: DVec3,
    /// Up direction. The ecliptic north pole, `+Z`, by default.
    pub up: DVec3,
    /// Vertical field of view, in radians.
    pub fov_y: f32,
    /// Near plane, in render units.
    ///
    /// With reverse-Z this can be tiny without wrecking depth precision, which
    /// is what lets the camera get close to a surface.
    pub near: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            position: DVec3::new(0.0, -3.0 * AU, 1.5 * AU),
            target: DVec3::ZERO,
            up: DVec3::Z,
            fov_y: 45_f32.to_radians(),
            near: 1e-7,
        }
    }
}

impl Camera {
    /// View matrix with the camera translated to the origin.
    ///
    /// The translation is applied on the CPU, in `f64`, when building instance
    /// data — see [`to_render_space`] — so the matrix itself is pure rotation.
    pub fn view(&self) -> Mat4 {
        let forward = (self.target - self.position).normalize_or_zero();
        let forward = if forward == DVec3::ZERO {
            Vec3::NEG_Z
        } else {
            forward.as_vec3()
        };
        let up = self.up.as_vec3();
        // Degenerate when looking straight along `up`; nudge it sideways.
        let up = if forward.cross(up).length_squared() < 1e-12 {
            Vec3::Y
        } else {
            up
        };
        glam::camera::rh::view::look_to_mat4(Vec3::ZERO, forward, up)
    }

    /// Reverse-Z perspective projection with an infinite far plane.
    ///
    /// Maps the near plane to depth 1 and infinity to depth 0. Pair it with
    /// [`wgpu::CompareFunction::Greater`] and a depth clear of `0.0`.
    pub fn projection(&self, aspect: f32) -> Mat4 {
        let focal = 1.0 / (self.fov_y * 0.5).tan();
        Mat4::from_cols_array_2d(&[
            [focal / aspect.max(1e-6), 0.0, 0.0, 0.0],
            [0.0, focal, 0.0, 0.0],
            [0.0, 0.0, 0.0, -1.0],
            [0.0, 0.0, self.near, 0.0],
        ])
    }

    /// Combined view-projection matrix, ready for the shader.
    pub fn view_projection(&self, aspect: f32) -> Mat4 {
        self.projection(aspect) * self.view()
    }
}

/// A camera that turns around a point of interest — the natural way to look at
/// a planetary system.
///
/// The focus is a world position in metres, so the controller can be pinned to
/// a moving body and simply follow it.
#[derive(Debug, Clone, Copy)]
pub struct OrbitCamera {
    /// The point being orbited, in metres.
    pub focus: DVec3,
    /// Distance from the focus, in metres.
    pub distance: f64,
    /// Rotation about the ecliptic pole, in radians.
    pub yaw: f64,
    /// Elevation above the ecliptic plane, in radians.
    pub pitch: f64,
    /// Vertical field of view, in radians.
    pub fov_y: f32,
    /// Closest the camera may get to the focus, in metres.
    ///
    /// Normally set to a small multiple of the focused body's radius so the
    /// camera cannot fall inside a planet.
    pub min_distance: f64,
    /// Furthest the camera may get, in metres.
    pub max_distance: f64,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            focus: DVec3::ZERO,
            distance: 5.0 * AU,
            yaw: 0.0,
            pitch: 0.45,
            fov_y: 45_f32.to_radians(),
            min_distance: 1e6,
            max_distance: 500.0 * AU,
        }
    }
}

impl OrbitCamera {
    /// Highest elevation the camera may reach.
    ///
    /// Stopping just short of the pole keeps the up vector well defined.
    const MAX_PITCH: f64 = std::f64::consts::FRAC_PI_2 - 1e-3;

    /// Eye position, in metres.
    pub fn eye(&self) -> DVec3 {
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        self.focus + self.distance * DVec3::new(cos_pitch * cos_yaw, cos_pitch * sin_yaw, sin_pitch)
    }

    /// The [`Camera`] this controller currently describes.
    pub fn camera(&self) -> Camera {
        Camera {
            position: self.eye(),
            target: self.focus,
            up: DVec3::Z,
            fov_y: self.fov_y,
            near: 1e-7,
        }
    }

    /// Rotates by the given deltas, in radians, clamping the elevation.
    pub fn orbit(&mut self, delta_yaw: f64, delta_pitch: f64) {
        self.yaw = (self.yaw + delta_yaw).rem_euclid(std::f64::consts::TAU);
        self.pitch = (self.pitch + delta_pitch).clamp(-Self::MAX_PITCH, Self::MAX_PITCH);
    }

    /// Multiplies the distance by `factor`, clamped to the configured range.
    ///
    /// Zooming multiplicatively rather than additively is what makes a single
    /// scroll wheel usable across twelve orders of magnitude: every notch
    /// covers the same *proportion* of the way in.
    pub fn zoom(&mut self, factor: f64) {
        self.distance = (self.distance * factor).clamp(self.min_distance, self.max_distance);
    }

    /// Frames a body of the given radius, in metres, from a comfortable
    /// distance.
    pub fn frame_body(&mut self, centre: DVec3, radius: f64) {
        self.focus = centre;
        self.min_distance = (radius * 1.2).max(1.0);
        self.distance = (radius * 6.0).clamp(self.min_distance, self.max_distance);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_camera_sits_at_the_requested_distance() {
        let controller = OrbitCamera {
            distance: 2.0 * AU,
            focus: DVec3::new(AU, 0.0, 0.0),
            ..OrbitCamera::default()
        };
        let offset = controller.eye() - controller.focus;
        assert!((offset.length() / (2.0 * AU) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn pitch_never_reaches_the_pole() {
        let mut controller = OrbitCamera::default();
        controller.orbit(0.0, 100.0);
        assert!(controller.pitch < std::f64::consts::FRAC_PI_2);
        controller.orbit(0.0, -200.0);
        assert!(controller.pitch > -std::f64::consts::FRAC_PI_2);
    }

    #[test]
    fn zoom_is_clamped_to_the_configured_range() {
        let mut controller = OrbitCamera::default();
        for _ in 0..200 {
            controller.zoom(0.5);
        }
        assert!((controller.distance - controller.min_distance).abs() < 1.0);
        for _ in 0..400 {
            controller.zoom(2.0);
        }
        assert!((controller.distance - controller.max_distance).abs() < 1.0);
    }

    /// Reverse-Z: the near plane must land on depth 1 and infinity on depth 0,
    /// with everything in between monotonically decreasing.
    #[test]
    fn the_projection_is_reverse_z_with_an_infinite_far_plane() {
        let camera = Camera::default();
        let projection = camera.projection(16.0 / 9.0);
        let depth_at = |distance: f32| {
            let clip = projection * glam::Vec4::new(0.0, 0.0, -distance, 1.0);
            clip.z / clip.w
        };

        assert!((depth_at(camera.near) - 1.0).abs() < 1e-5);
        assert!(depth_at(1e9) < 1e-9, "distant geometry should approach 0");

        let mut previous = f32::INFINITY;
        for exponent in -6..9 {
            let depth = depth_at(10_f32.powi(exponent));
            assert!(depth < previous, "depth must fall with distance");
            previous = depth;
        }
    }

    #[test]
    fn render_space_keeps_precision_at_neptune() {
        // A point one Neptune-radius from a body 30 AU away, viewed from 30 AU
        // away, must still be resolved far more finely than the body itself.
        let camera_position = DVec3::new(30.0 * AU, 0.0, 0.0);
        let surface = camera_position + DVec3::new(2.4622e7, 0.0, 0.0);
        let rendered = to_render_space(surface, camera_position);
        let expected = 2.4622e7 / AU;
        assert!((f64::from(rendered.x) / expected - 1.0).abs() < 1e-5);
    }
}
