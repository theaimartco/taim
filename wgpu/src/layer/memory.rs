use super::Layer;
use crate::graphics::Mesh;
use crate::text::Text;

const MAX_RECYCLED_GROUPS: usize = 8;
const MAX_RECYCLED_GROUP_BYTES: usize = 256 * 1024;

impl Layer {
    pub(super) fn recycle_meshes(&mut self, mut meshes: Vec<Mesh>) {
        meshes.clear();
        if self.mesh_pool.len() < MAX_RECYCLED_GROUPS
            && meshes
                .capacity()
                .saturating_mul(std::mem::size_of::<Mesh>())
                <= MAX_RECYCLED_GROUP_BYTES
        {
            self.mesh_pool.push(meshes);
        }
    }

    pub(super) fn recycle_text(&mut self, mut text: Vec<Text>) {
        text.clear();
        if self.text_pool.len() < MAX_RECYCLED_GROUPS
            && text.capacity().saturating_mul(std::mem::size_of::<Text>())
                <= MAX_RECYCLED_GROUP_BYTES
        {
            self.text_pool.push(text);
        }
    }

    pub(super) fn retained_bytes(&self) -> usize {
        let triangle_contents = self
            .triangles
            .iter()
            .map(|item| match item {
                crate::triangle::Item::Group { meshes, .. } => meshes
                    .capacity()
                    .saturating_mul(std::mem::size_of::<Mesh>()),
                crate::triangle::Item::Cached { .. } => 0,
            })
            .sum::<usize>();
        let text_contents = self
            .text
            .iter()
            .map(|item| match item {
                crate::text::Item::Group { text, .. } => {
                    text.capacity().saturating_mul(std::mem::size_of::<Text>())
                }
                crate::text::Item::Cached { .. } => 0,
            })
            .sum::<usize>();
        let mesh_pool = self
            .mesh_pool
            .iter()
            .map(|meshes| {
                meshes
                    .capacity()
                    .saturating_mul(std::mem::size_of::<Mesh>())
            })
            .sum::<usize>();
        let text_pool = self
            .text_pool
            .iter()
            .map(|text| text.capacity().saturating_mul(std::mem::size_of::<Text>()))
            .sum::<usize>();

        #[cfg(any(feature = "svg", feature = "image"))]
        let image_bytes = self
            .images
            .capacity()
            .saturating_mul(std::mem::size_of::<crate::image::Image>());
        #[cfg(not(any(feature = "svg", feature = "image")))]
        let image_bytes = std::mem::size_of_val(&self.images);

        std::mem::size_of::<Self>()
            + self.quads.estimated_bytes()
            + self
                .triangles
                .capacity()
                .saturating_mul(std::mem::size_of::<crate::triangle::Item>())
            + triangle_contents
            + self
                .primitives
                .capacity()
                .saturating_mul(std::mem::size_of::<crate::primitive::Instance>())
            + image_bytes
            + self
                .text
                .capacity()
                .saturating_mul(std::mem::size_of::<crate::text::Item>())
            + text_contents
            + self
                .pending_meshes
                .capacity()
                .saturating_mul(std::mem::size_of::<Mesh>())
            + self
                .pending_text
                .capacity()
                .saturating_mul(std::mem::size_of::<Text>())
            + self
                .mesh_pool
                .capacity()
                .saturating_mul(std::mem::size_of::<Vec<Mesh>>())
            + self
                .text_pool
                .capacity()
                .saturating_mul(std::mem::size_of::<Vec<Text>>())
            + mesh_pool
            + text_pool
    }
}
