//! Own GLB writer on `gltf-json`: one JSON chunk, one binary chunk, one buffer.

use gltf_json::accessor::{ComponentType, GenericComponentType, Type};
use gltf_json::buffer::{Target, View};
use gltf_json::validation::{Checked::Valid, USize64};
use gltf_json::{Accessor, Buffer, Index, Root};

const MAGIC: u32 = 0x4654_6C67; // "glTF"
const CHUNK_JSON: u32 = 0x4E4F_534A;
const CHUNK_BIN: u32 = 0x004E_4942;

/// Collects a glTF document and its binary data, then writes one `.glb`.
pub struct GlbWriter {
    pub root: Root,
    bin: Vec<u8>,
}

impl Default for GlbWriter {
    fn default() -> Self {
        let mut root = Root::default();
        root.asset.generator = Some(format!("Quarry {}", env!("CARGO_PKG_VERSION")));
        root.buffers.push(Buffer {
            byte_length: USize64(0),
            name: None,
            uri: None,
            extensions: None,
            extras: Default::default(),
        });
        Self {
            root,
            bin: Vec::new(),
        }
    }
}

impl GlbWriter {
    /// Store `data` in its own buffer view and describe it with an accessor.
    /// `bounds` adds min/max, which glTF requires for POSITION and animation input.
    pub fn accessor<T: bytemuck::Pod>(
        &mut self,
        data: &[T],
        component: ComponentType,
        type_: Type,
        target: Option<Target>,
        bounds: bool,
    ) -> Index<Accessor> {
        let bytes: &[u8] = bytemuck::cast_slice(data);
        let width = type_.multiplicity() * component.size();
        assert!(
            bytes.len().is_multiple_of(width),
            "data is not a whole number of elements"
        );
        let count = bytes.len() / width;

        let (min, max) = if bounds {
            assert_eq!(
                component,
                ComponentType::F32,
                "bounds are only written for floats"
            );
            let floats: &[f32] = bytemuck::cast_slice(bytes);
            let n = type_.multiplicity();
            let mut min = vec![f32::INFINITY; n];
            let mut max = vec![f32::NEG_INFINITY; n];
            for element in floats.chunks_exact(n) {
                for (i, v) in element.iter().enumerate() {
                    min[i] = min[i].min(*v);
                    max[i] = max[i].max(*v);
                }
            }
            (Some(min.into()), Some(max.into()))
        } else {
            (None, None)
        };

        while !self.bin.len().is_multiple_of(4) {
            self.bin.push(0);
        }
        let view = self.root.push(View {
            buffer: Index::new(0),
            byte_length: bytes.len().into(),
            byte_offset: Some(self.bin.len().into()),
            byte_stride: None,
            name: None,
            target: target.map(Valid),
            extensions: None,
            extras: Default::default(),
        });
        self.bin.extend_from_slice(bytes);

        self.root.push(Accessor {
            buffer_view: Some(view),
            byte_offset: None,
            count: count.into(),
            component_type: Valid(GenericComponentType(component)),
            extensions: None,
            extras: Default::default(),
            type_: Valid(type_),
            min,
            max,
            name: None,
            normalized: false,
            sparse: None,
        })
    }

    /// The finished `.glb` bytes.
    pub fn finish(mut self) -> Result<Vec<u8>, serde_json::Error> {
        while !self.bin.len().is_multiple_of(4) {
            self.bin.push(0);
        }
        self.root.buffers[0].byte_length = self.bin.len().into();

        let mut json = self.root.to_vec()?;
        while !json.len().is_multiple_of(4) {
            json.push(b' ');
        }

        let total = 12 + 8 + json.len() + 8 + self.bin.len();
        let mut out = Vec::with_capacity(total);
        for word in [MAGIC, 2, total as u32, json.len() as u32, CHUNK_JSON] {
            out.extend_from_slice(&word.to_le_bytes());
        }
        out.extend_from_slice(&json);
        for word in [self.bin.len() as u32, CHUNK_BIN] {
            out.extend_from_slice(&word.to_le_bytes());
        }
        out.extend_from_slice(&self.bin);
        Ok(out)
    }
}
