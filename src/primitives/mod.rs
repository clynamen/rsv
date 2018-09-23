#[derive(Copy, Clone)]
pub struct Vertex {
    pub position: (f32, f32, f32)
}

impl_vertex!(Vertex, position);