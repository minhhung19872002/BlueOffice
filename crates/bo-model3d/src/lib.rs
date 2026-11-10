//! 3D models for documents (CORE-3DMODEL-001).
//!
//! * [`gltf`]: glTF 2.0 / GLB reader (Khronos glTF 2.0 specification: meshes, accessors,
//!   nodes with matrix / TRS transforms, PBR metallic-roughness base colour and texture).
//! * [`obj`] / [`stl`]: Wavefront OBJ and STL (ASCII + binary) readers.
//! * [`glb`]: GLB writer — every inserted model is stored as GLB (`model/gltf-binary`), the
//!   format the [MS-ODRAWXML] `am3d:model3d` part holds.
//! * [`view`]: the scene / camera / viewport of `am3d:model3d` ([MS-ODRAWXML] model3d schema)
//!   as numbers, and the projection used by the renderer and the web viewer.
//! * [`raster`]: a deterministic software z-buffer renderer (picture fallback, previews and
//!   exports).
//! * [`sample`]: small self-made sample models (corpus, tests).
#![forbid(unsafe_code)]

pub mod glb;
pub mod gltf;
pub mod obj;
pub mod raster;
pub mod sample;
pub mod scene;
pub mod stl;
pub mod view;

pub use scene::{Material, Prim, Scene, Texture};

/// Errors of the model readers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelError {
    /// The bytes are not a model of the expected format.
    Format(&'static str),
    /// A feature the reader does not implement (external buffers, mesh compression…).
    Unsupported(&'static str),
    /// More geometry than the limits allow.
    TooLarge,
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelError::Format(s) => write!(f, "bad model: {s}"),
            ModelError::Unsupported(s) => write!(f, "unsupported model feature: {s}"),
            ModelError::TooLarge => write!(f, "model too large"),
        }
    }
}

impl std::error::Error for ModelError {}

/// Result alias.
pub type Result<T> = std::result::Result<T, ModelError>;

/// Most triangles a model may have (all primitives).
pub const MAX_TRIANGLES: usize = 2_000_000;

/// Model file kinds, by file name extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Binary glTF (`.glb`).
    Glb,
    /// glTF JSON with embedded (`data:`) buffers (`.gltf`).
    Gltf,
    /// Wavefront OBJ (`.obj`).
    Obj,
    /// STL (`.stl`).
    Stl,
}

impl Kind {
    /// Kind of a file name (case-insensitive extension).
    pub fn of_name(name: &str) -> Option<Kind> {
        let ext = name.rsplit('.').next()?.to_ascii_lowercase();
        Some(match ext.as_str() {
            "glb" => Kind::Glb,
            "gltf" => Kind::Gltf,
            "obj" => Kind::Obj,
            "stl" => Kind::Stl,
            _ => return None,
        })
    }
}

/// Is `name` a model file BlueOffice can insert?
pub fn is_model_name(name: &str) -> bool {
    Kind::of_name(name).is_some()
}

/// Read a model file and give its scene and the GLB bytes to store: a GLB is kept as is,
/// the other kinds are converted ([`glb::write`]).
pub fn load(bytes: &[u8], name: &str) -> Result<(Scene, Vec<u8>)> {
    let kind = if bytes.starts_with(b"glTF") {
        Kind::Glb
    } else {
        Kind::of_name(name).ok_or(ModelError::Format("unknown kind"))?
    };
    match kind {
        Kind::Glb => {
            let scene = gltf::parse_glb(bytes)?;
            Ok((scene, bytes.to_vec()))
        }
        Kind::Gltf => {
            let scene = gltf::parse_gltf(bytes, None)?;
            let out = glb::write(&scene);
            Ok((scene, out))
        }
        Kind::Obj => {
            let scene = obj::parse(bytes)?;
            let out = glb::write(&scene);
            Ok((scene, out))
        }
        Kind::Stl => {
            let scene = stl::parse(bytes)?;
            let out = glb::write(&scene);
            Ok((scene, out))
        }
    }
}
