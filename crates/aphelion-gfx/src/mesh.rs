//! Procedural geometry.
//!
//! Every body is the same unit sphere, scaled and positioned by its instance
//! transform, so there is exactly one mesh on the GPU no matter how many
//! planets are on screen.

use bytemuck::{Pod, Zeroable};

/// One vertex of a mesh.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct Vertex {
    /// Position, in model space.
    pub position: [f32; 3],
    /// Outward normal.
    pub normal: [f32; 3],
    /// Texture coordinates: `u` is longitude, `v` is latitude from the south
    /// pole.
    pub uv: [f32; 2],
}

impl Vertex {
    /// Vertex buffer layout matching `body.wgsl`.
    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: size_of::<Vertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x2],
    };
}

/// CPU-side triangle mesh.
#[derive(Debug, Clone, Default)]
pub struct MeshData {
    /// Vertices.
    pub vertices: Vec<Vertex>,
    /// Triangle indices.
    pub indices: Vec<u32>,
}

impl MeshData {
    /// Number of triangles.
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
}

/// Builds a unit sphere as a latitude/longitude grid.
///
/// `rings` is the number of latitude bands and `segments` the number of
/// longitude divisions; 48 × 96 is smooth enough to fill the screen without
/// visible faceting, at about 9000 triangles.
///
/// The seam is duplicated so that `u` runs cleanly from 0 to 1 instead of
/// wrapping, and the poles get their own rows of vertices so their texture
/// coordinates are well defined.
///
/// # Panics
///
/// Panics if `rings` or `segments` is less than 3.
pub fn uv_sphere(rings: u32, segments: u32) -> MeshData {
    assert!(rings >= 3 && segments >= 3, "a sphere needs at least 3x3");

    let mut vertices = Vec::with_capacity(((rings + 1) * (segments + 1)) as usize);
    for ring in 0..=rings {
        let v = ring as f32 / rings as f32;
        // Latitude from the south pole (v = 0) to the north pole (v = 1).
        let polar = v * std::f32::consts::PI;
        let (sin_polar, cos_polar) = polar.sin_cos();

        for segment in 0..=segments {
            let u = segment as f32 / segments as f32;
            let azimuth = u * std::f32::consts::TAU;
            let (sin_azimuth, cos_azimuth) = azimuth.sin_cos();

            // +Z is the rotation axis, matching the simulation's ecliptic frame.
            let normal = [sin_polar * cos_azimuth, sin_polar * sin_azimuth, -cos_polar];
            vertices.push(Vertex {
                position: normal,
                normal,
                uv: [u, v],
            });
        }
    }

    let row = segments + 1;
    let mut indices = Vec::with_capacity((rings * segments * 6) as usize);
    for ring in 0..rings {
        for segment in 0..segments {
            let bottom_left = ring * row + segment;
            let bottom_right = bottom_left + 1;
            let top_left = bottom_left + row;
            let top_right = top_left + 1;

            // Counter-clockwise when seen from outside.
            indices.extend_from_slice(&[bottom_left, top_left, bottom_right]);
            indices.extend_from_slice(&[bottom_right, top_left, top_right]);
        }
    }

    MeshData { vertices, indices }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sphere_has_the_expected_topology() {
        let mesh = uv_sphere(16, 32);
        assert_eq!(mesh.vertices.len(), 17 * 33);
        assert_eq!(mesh.triangle_count(), 16 * 32 * 2);
        assert!(
            mesh.indices
                .iter()
                .all(|&i| (i as usize) < mesh.vertices.len())
        );
    }

    #[test]
    fn every_vertex_lies_on_the_unit_sphere() {
        for vertex in uv_sphere(24, 48).vertices {
            let [x, y, z] = vertex.position;
            let radius = (x * x + y * y + z * z).sqrt();
            assert!((radius - 1.0).abs() < 1e-5, "radius {radius}");
            // Bit-identical by construction: on a unit sphere the outward
            // normal *is* the position, assigned from the same expression.
            #[allow(clippy::float_cmp)]
            {
                assert!(
                    vertex.position == vertex.normal,
                    "normals should face outward"
                );
            }
        }
    }

    #[test]
    fn the_poles_are_where_they_should_be() {
        let mesh = uv_sphere(8, 16);
        // v = 0 is the south pole, v = 1 the north pole.
        assert!((mesh.vertices[0].position[2] + 1.0).abs() < 1e-6);
        assert!((mesh.vertices.last().unwrap().position[2] - 1.0).abs() < 1e-6);
    }
}
