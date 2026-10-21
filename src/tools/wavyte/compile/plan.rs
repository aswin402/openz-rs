use std::collections::HashMap;

use crate::tools::wavyte::{
    assets::store::{AssetId, PreparedAsset, PreparedAssetStore},
    composition::model::{BlendMode, Composition},
    effects::fx::{PassFx, normalize_effects, parse_effect},
    effects::transitions::{TransitionKind, WipeDir, parse_transition_kind_params},
    eval::EvaluatedGraph,
    foundation::core::{Affine, BezPath, Canvas, Rgba8Premul},
    foundation::error::WavyteResult,
    foundation::math::{
        Fnv1a64, write_i64_pair, write_str_pair, write_u8_pair, write_u64_pair,
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct JsonFingerprint {
    hi: u64,
    lo: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct EffectCacheKey {
    kind_hash: u64,
    params_fingerprint: JsonFingerprint,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct TransitionCacheKey {
    kind_hash: u64,
    params_fingerprint: JsonFingerprint,
}

#[derive(Clone, Debug)]
struct EffectCacheEntry {
    kind: String,
    params: serde_json::Value,
    parsed: crate::tools::wavyte::effects::fx::Effect,
}

#[derive(Clone, Debug)]
struct TransitionCacheEntry {
    kind: String,
    params: serde_json::Value,
    parsed: TransitionKind,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct CompileCache {
    effect_cache: HashMap<EffectCacheKey, Vec<EffectCacheEntry>>,
    transition_cache: HashMap<TransitionCacheKey, Vec<TransitionCacheEntry>>,
}

#[derive(Clone, Debug)]
/// Backend-agnostic render plan for a single frame.
///
/// A plan consists of:
/// - surface declarations (`surfaces`)
/// - a sequence of passes (`passes`)
/// - a declared final surface (`final_surface`)
///
/// The plan is designed to be executable by multiple backends (CPU and GPU) with the same
/// semantics.
pub struct RenderPlan {
    /// Target canvas metadata for this frame.
    pub canvas: Canvas,
    /// Surface declarations used by pass execution.
    pub surfaces: Vec<SurfaceDesc>,
    /// Ordered pass list to execute.
    pub passes: Vec<Pass>,
    /// Surface to read back as final frame.
    pub final_surface: SurfaceId,
}

#[derive(Clone, Debug)]
/// A single pass in a [`RenderPlan`].
pub enum Pass {
    /// Draw source content to a surface.
    Scene(ScenePass),
    /// Run an effect pass from input surface to output surface.
    Offscreen(OffscreenPass),
    /// Composite multiple surfaces into target.
    Composite(CompositePass),
}

#[derive(Clone, Debug)]
/// Draw operations into a surface.
pub struct ScenePass {
    /// Target surface for all draw ops in this pass.
    pub target: SurfaceId,
    /// Draw operations in this scene pass.
    pub ops: Vec<DrawOp>,
    /// Clear target to transparent before drawing when `true`.
    pub clear_to_transparent: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
/// Identifier for a render surface declared in [`RenderPlan::surfaces`].
pub struct SurfaceId(
    /// Raw surface index in [`RenderPlan::surfaces`].
    pub u32,
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Supported pixel formats for render surfaces.
pub enum PixelFormat {
    /// Premultiplied RGBA8 pixel format.
    Rgba8Premul,
}

#[derive(Clone, Debug, PartialEq, Eq)]
/// Surface declaration: dimensions + pixel format.
pub struct SurfaceDesc {
    /// Surface width in pixels.
    pub width: u32,
    /// Surface height in pixels.
    pub height: u32,
    /// Surface pixel format.
    pub format: PixelFormat,
}

#[derive(Clone, Debug)]
/// Run a post-processing effect producing a new surface from an input surface.
pub struct OffscreenPass {
    /// Input surface.
    pub input: SurfaceId,
    /// Output surface.
    pub output: SurfaceId,
    /// Effect operation to run.
    pub fx: PassFx,
}

#[derive(Clone, Debug)]
/// Composite multiple surfaces into a target surface.
pub struct CompositePass {
    /// Output surface.
    pub target: SurfaceId,
    /// Ordered compositing operations.
    pub ops: Vec<CompositeOp>,
}

#[derive(Clone, Debug)]
/// A compositing operation between surfaces.
pub enum CompositeOp {
    /// Alpha-over `src` onto target with additional opacity multiplier.
    Over {
        /// Source surface.
        src: SurfaceId,
        /// Extra opacity multiplier in `[0, 1]`.
        opacity: f32,
    },
    /// Crossfade between two source surfaces.
    Crossfade {
        /// Outgoing surface.
        a: SurfaceId,
        /// Incoming surface.
        b: SurfaceId,
        /// Crossfade factor in `[0, 1]`.
        t: f32,
    },
    /// Directional wipe between two surfaces.
    Wipe {
        /// Outgoing surface.
        a: SurfaceId,
        /// Incoming surface.
        b: SurfaceId,
        /// Wipe progress in `[0, 1]`.
        t: f32,
        /// Wipe direction.
        dir: WipeDir,
        /// Edge softness in `[0, 1]`.
        soft_edge: f32,
    },
}

#[derive(Clone, Debug)]
/// Draw operation emitted by the compiler.
pub enum DrawOp {
    /// Fill vector path geometry.
    FillPath {
        /// Path geometry in local space.
        path: BezPath,
        /// Local-to-canvas transform.
        transform: Affine,
        /// Fill color in premultiplied RGBA8.
        color: Rgba8Premul,
        /// Opacity multiplier in `[0, 1]`.
        opacity: f32,
        /// Blend mode.
        blend: BlendMode,
        /// Draw order key.
        z: i32,
    },
    /// Draw prepared bitmap image asset.
    Image {
        /// Prepared asset identifier.
        asset: AssetId,
        /// Local-to-canvas transform.
        transform: Affine,
        /// Opacity multiplier in `[0, 1]`.
        opacity: f32,
        /// Blend mode.
        blend: BlendMode,
        /// Draw order key.
        z: i32,
    },
    /// Draw prepared SVG asset.
    Svg {
        /// Prepared asset identifier.
        asset: AssetId,
        /// Local-to-canvas transform.
        transform: Affine,
        /// Opacity multiplier in `[0, 1]`.
        opacity: f32,
        /// Blend mode.
        blend: BlendMode,
        /// Draw order key.
        z: i32,
    },
    /// Draw prepared text asset.
    Text {
        /// Prepared asset identifier.
        asset: AssetId,
        /// Local-to-canvas transform.
        transform: Affine,
        /// Opacity multiplier in `[0, 1]`.
        opacity: f32,
        /// Blend mode.
        blend: BlendMode,
        /// Draw order key.
        z: i32,
    },
    /// Draw decoded frame from prepared video asset.
    Video {
        /// Prepared asset identifier.
        asset: AssetId,
        /// Source media time in seconds.
        source_time_s: f64,
        /// Local-to-canvas transform.
        transform: Affine,
        /// Opacity multiplier in `[0, 1]`.
        opacity: f32,
        /// Blend mode.
        blend: BlendMode,
        /// Draw order key.
        z: i32,
    },
}

/// Compile one evaluated frame graph into backend-agnostic render plan.
pub fn compile_frame(
    comp: &Composition,
    eval: &EvaluatedGraph,
    assets: &PreparedAssetStore,
) -> WavyteResult<RenderPlan> {
    let mut cache = CompileCache::default();
    compile_frame_with_cache(comp, eval, assets, &mut cache)
}

pub(crate) fn compile_frame_with_cache(
    comp: &Composition,
    eval: &EvaluatedGraph,
    assets: &PreparedAssetStore,
    cache: &mut CompileCache,
) -> WavyteResult<RenderPlan> {
    #[derive(Clone, Debug)]
    struct Layer {
        surface: SurfaceId,
        transition_in: Option<crate::tools::wavyte::eval::ResolvedTransition>,
        transition_out: Option<crate::tools::wavyte::eval::ResolvedTransition>,
    }

    let mut surfaces = Vec::<SurfaceDesc>::new();
    surfaces.push(SurfaceDesc {
        width: comp.canvas.width,
        height: comp.canvas.height,
        format: PixelFormat::Rgba8Premul,
    });

    let mut scene_passes = Vec::<Pass>::with_capacity(eval.nodes.len());
    let mut layers = Vec::<Layer>::with_capacity(eval.nodes.len());

    for (idx, node) in eval.nodes.iter().enumerate() {
        let mut parsed = Vec::with_capacity(node.effects.len());
        for e in &node.effects {
            parsed.push(parse_effect_cached(cache, e)?);
        }
        let fx = normalize_effects(&parsed);

        // Transitions are handled during composition. Keep DrawOp opacity for "intrinsic" opacity
        // only (clip opacity + inline opacity effect).
        let opacity = ((node.opacity as f32) * fx.inline.opacity_mul).clamp(0.0, 1.0);

        match opacity <= 0.0 {
            true => continue,
            false => {}
        }

        let transform = node.transform * fx.inline.transform_post;

        let asset_id = assets.id_for_key(&node.asset)?;
        let op = match assets.get(asset_id)? {
            PreparedAsset::Path(a) => DrawOp::FillPath {
                path: a.path.clone(),
                transform,
                color: Rgba8Premul::from_straight_rgba(255, 255, 255, 255),
                opacity,
                blend: node.blend,
                z: node.z,
            },
            PreparedAsset::Image(_) => DrawOp::Image {
                asset: asset_id,
                transform,
                opacity,
                blend: node.blend,
                z: node.z,
            },
            PreparedAsset::Svg(_) => DrawOp::Svg {
                asset: asset_id,
                transform,
                opacity,
                blend: node.blend,
                z: node.z,
            },
            PreparedAsset::Text(_) => DrawOp::Text {
                asset: asset_id,
                transform,
                opacity,
                blend: node.blend,
                z: node.z,
            },
            PreparedAsset::Video(_) => DrawOp::Video {
                asset: asset_id,
                source_time_s: node.source_time_s.unwrap_or(0.0),
                transform,
                opacity,
                blend: node.blend,
                z: node.z,
            },
            PreparedAsset::Audio(_) => continue,
        };

        let surf_id = SurfaceId((surfaces.len()) as u32);
        surfaces.push(SurfaceDesc {
            width: comp.canvas.width,
            height: comp.canvas.height,
            format: PixelFormat::Rgba8Premul,
        });

        scene_passes.push(Pass::Scene(ScenePass {
            target: surf_id,
            ops: vec![op],
            clear_to_transparent: true,
        }));

        let mut post_fx = surf_id;
        for fx in &fx.passes {
            let out_id = SurfaceId((surfaces.len()) as u32);
            surfaces.push(SurfaceDesc {
                width: comp.canvas.width,
                height: comp.canvas.height,
                format: PixelFormat::Rgba8Premul,
            });
            scene_passes.push(Pass::Offscreen(OffscreenPass {
                input: post_fx,
                output: out_id,
                fx: fx.clone(),
            }));
            post_fx = out_id;
        }

        let _ = idx;
        layers.push(Layer {
            surface: post_fx,
            transition_in: node.transition_in.clone(),
            transition_out: node.transition_out.clone(),
        });
    }

    let mut composite_ops = Vec::<CompositeOp>::with_capacity(layers.len());
    let mut i = 0usize;
    while i < layers.len() {
        let layer = &layers[i];

        let paired_op = layers.get(i + 1).and_then(|next| {
            let out_tr = layer.transition_out.as_ref()?;
            let in_tr = next.transition_in.as_ref()?;
            let out_kind = parse_transition_cached(cache, out_tr).ok()?;
            let in_kind = parse_transition_cached(cache, in_tr).ok()?;
            let t_in = (in_tr.progress as f32).clamp(0.0, 1.0);
            let t_out = (out_tr.progress as f32).clamp(0.0, 1.0);

            // Explicit v0.2 pairing rule: the Out and In edges must agree on progress
            // (same duration/ease and overlapping window).
            let progress_close = (t_in - t_out).abs() <= 0.05;
            progress_close
                .then(|| match (out_kind, in_kind) {
                    (TransitionKind::Crossfade, TransitionKind::Crossfade) => {
                        Some(CompositeOp::Crossfade {
                            a: layer.surface,
                            b: next.surface,
                            t: t_in,
                        })
                    }
                    (
                        TransitionKind::Wipe {
                            dir: dir_a,
                            soft_edge: soft_a,
                        },
                        TransitionKind::Wipe {
                            dir: dir_b,
                            soft_edge: soft_b,
                        },
                    ) => (dir_a == dir_b && (soft_a - soft_b).abs() <= 1e-6).then_some(
                        CompositeOp::Wipe {
                            a: layer.surface,
                            b: next.surface,
                            t: t_in,
                            dir: dir_a,
                            soft_edge: soft_a,
                        },
                    ),
                    _ => None,
                })
                .flatten()
        });

        match paired_op {
            Some(op) => {
                composite_ops.push(op);
                i += 2;
            }
            None => {
                let mut layer_opacity = 1.0f32;
                match &layer.transition_in {
                    Some(tr) => layer_opacity *= tr.progress as f32,
                    None => {}
                }
                match &layer.transition_out {
                    Some(tr) => layer_opacity *= (1.0 - tr.progress) as f32,
                    None => {}
                }
                layer_opacity = layer_opacity.clamp(0.0, 1.0);

                match layer_opacity > 0.0 {
                    true => composite_ops.push(CompositeOp::Over {
                        src: layer.surface,
                        opacity: layer_opacity,
                    }),
                    false => {}
                }

                i += 1;
            }
        }
    }

    Ok(RenderPlan {
        canvas: comp.canvas,
        surfaces,
        passes: {
            let mut out = scene_passes;
            out.push(Pass::Composite(CompositePass {
                target: SurfaceId(0),
                ops: composite_ops,
            }));
            out
        },
        final_surface: SurfaceId(0),
    })
}

fn parse_effect_cached(
    cache: &mut CompileCache,
    effect: &crate::tools::wavyte::eval::ResolvedEffect,
) -> WavyteResult<crate::tools::wavyte::effects::fx::Effect> {
    let key = EffectCacheKey {
        kind_hash: hash_str64(&effect.kind),
        params_fingerprint: fingerprint_json_value(&effect.params),
    };

    let cached = cache.effect_cache.get(&key).and_then(|bucket| {
        bucket
            .iter()
            .find(|entry| entry.kind == effect.kind && entry.params == effect.params)
            .map(|found| found.parsed.clone())
    });
    match cached {
        Some(found) => Ok(found),
        None => {
            let parsed = parse_effect(&crate::tools::wavyte::composition::model::EffectInstance {
                kind: effect.kind.clone(),
                params: effect.params.clone(),
            })?;
            cache
                .effect_cache
                .entry(key)
                .or_default()
                .push(EffectCacheEntry {
                    kind: effect.kind.clone(),
                    params: effect.params.clone(),
                    parsed: parsed.clone(),
                });
            Ok(parsed)
        }
    }
}

fn parse_transition_cached(
    cache: &mut CompileCache,
    transition: &crate::tools::wavyte::eval::ResolvedTransition,
) -> WavyteResult<TransitionKind> {
    let key = TransitionCacheKey {
        kind_hash: hash_str64(&transition.kind),
        params_fingerprint: fingerprint_json_value(&transition.params),
    };

    let cached = cache.transition_cache.get(&key).and_then(|bucket| {
        bucket
            .iter()
            .find(|entry| entry.kind == transition.kind && entry.params == transition.params)
            .map(|found| found.parsed.clone())
    });
    match cached {
        Some(found) => Ok(found),
        None => {
            let parsed = parse_transition_kind_params(&transition.kind, &transition.params)?;
            cache
                .transition_cache
                .entry(key)
                .or_default()
                .push(TransitionCacheEntry {
                    kind: transition.kind.clone(),
                    params: transition.params.clone(),
                    parsed: parsed.clone(),
                });
            Ok(parsed)
        }
    }
}

fn hash_str64(value: &str) -> u64 {
    let mut h = Fnv1a64::new_default();
    h.write_bytes(value.as_bytes());
    h.finish()
}

fn fingerprint_json_value(v: &serde_json::Value) -> JsonFingerprint {
    let mut a = Fnv1a64::new(0xcbf29ce484222325);
    let mut b = Fnv1a64::new(0x9ae16a3b2f90404f);
    hash_json_value_pair(&mut a, &mut b, v);
    JsonFingerprint {
        hi: a.finish(),
        lo: b.finish(),
    }
}

fn hash_json_value_pair(a: &mut Fnv1a64, b: &mut Fnv1a64, v: &serde_json::Value) {
    match v {
        serde_json::Value::Null => write_u8_pair(a, b, 0),
        serde_json::Value::Bool(x) => {
            write_u8_pair(a, b, 1);
            write_u8_pair(a, b, u8::from(*x));
        }
        serde_json::Value::Number(n) => {
            write_u8_pair(a, b, 2);
            match (n.as_i64(), n.as_u64(), n.as_f64()) {
                (Some(i), _, _) => {
                    write_u8_pair(a, b, 0);
                    write_i64_pair(a, b, i);
                }
                (None, Some(u), _) => {
                    write_u8_pair(a, b, 1);
                    write_u64_pair(a, b, u);
                }
                (None, None, Some(f)) => {
                    write_u8_pair(a, b, 2);
                    write_u64_pair(a, b, f.to_bits());
                }
                (None, None, None) => {
                    write_u8_pair(a, b, 3);
                    write_str_pair(a, b, &n.to_string());
                }
            }
        }
        serde_json::Value::String(s) => {
            write_u8_pair(a, b, 3);
            write_str_pair(a, b, s);
        }
        serde_json::Value::Array(items) => {
            write_u8_pair(a, b, 4);
            write_u64_pair(a, b, items.len() as u64);
            for item in items {
                hash_json_value_pair(a, b, item);
            }
        }
        serde_json::Value::Object(map) => {
            write_u8_pair(a, b, 5);
            let mut keys = map.keys().collect::<Vec<_>>();
            keys.sort_unstable();
            write_u64_pair(a, b, keys.len() as u64);
            for k in keys {
                write_str_pair(a, b, k);
                hash_json_value_pair(a, b, &map[k]);
            }
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/compile/plan.rs"]
mod tests;
