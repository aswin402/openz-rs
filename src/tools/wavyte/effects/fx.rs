use crate::tools::wavyte::{
    composition::model::EffectInstance,
    foundation::core::Affine,
    foundation::error::{WavyteError, WavyteResult},
};

#[derive(Clone, Debug, PartialEq)]
/// Parsed effect representation used by compiler normalization.
pub enum Effect {
    /// Multiply clip opacity by `value`.
    OpacityMul {
        /// Multiplicative opacity factor.
        value: f32,
    },
    /// Post-multiply clip transform with an affine matrix.
    TransformPost {
        /// Affine matrix post-multiplied onto clip transform.
        value: Affine,
    },
    /// Gaussian blur pass parameters.
    Blur {
        /// Blur radius in pixels.
        radius_px: u32,
        /// Standard deviation in pixels.
        sigma: f32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
/// Inline effects folded directly into draw operation state.
pub struct InlineFx {
    /// Multiplicative opacity factor accumulated from effects.
    pub opacity_mul: f32,
    /// Post-transform matrix accumulated from effects.
    pub transform_post: Affine,
}

impl Default for InlineFx {
    fn default() -> Self {
        Self {
            opacity_mul: 1.0,
            transform_post: Affine::IDENTITY,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
/// Effects that require explicit offscreen render passes.
pub enum PassFx {
    /// Gaussian blur applied to a surface.
    Blur {
        /// Blur radius in pixels.
        radius_px: u32,
        /// Standard deviation in pixels.
        sigma: f32,
    },
}

#[derive(Clone, Debug, Default, PartialEq)]
/// Normalized effect pipeline for one clip.
pub struct FxPipeline {
    /// Inline adjustments applied at compile time.
    pub inline: InlineFx,
    /// Ordered offscreen passes to execute after scene draw.
    pub passes: Vec<PassFx>,
}

/// Parse a user-provided effect instance into typed effect data.
pub fn parse_effect(inst: &EffectInstance) -> WavyteResult<Effect> {
    let kind = inst.kind.trim().to_ascii_lowercase();
    (!kind.is_empty())
        .then_some(())
        .ok_or_else(|| WavyteError::validation("effect kind must be non-empty"))?;

    match kind.as_str() {
        "opacitymul" | "opacity_mul" | "opacity-mul" => {
            let value = get_f32(&inst.params, "value")?;
            (value.is_finite() && value >= 0.0)
                .then_some(Effect::OpacityMul { value })
                .ok_or_else(|| {
                    WavyteError::validation("OpacityMul.value must be finite and >= 0")
                })
        }
        "transformpost" | "transform_post" | "transform-post" => {
            let value = parse_affine(&inst.params)?;
            Ok(Effect::TransformPost { value })
        }
        "blur" => {
            let radius_px = get_u32(&inst.params, "radius_px")?;
            (radius_px <= 256).then_some(()).ok_or_else(|| {
                WavyteError::validation("Blur.radius_px must be <= 256 in v0.2.1")
            })?;
            let sigma = match inst.params.get("sigma") {
                Some(v) => {
                    let s = v
                        .as_f64()
                        .ok_or_else(|| WavyteError::validation("Blur.sigma must be a number"))?
                        as f32;
                    (s.is_finite() && s > 0.0).then_some(s).ok_or_else(|| {
                        WavyteError::validation("Blur.sigma must be finite and > 0")
                    })?
                }
                None => (radius_px as f32) / 2.0,
            };
            Ok(Effect::Blur { radius_px, sigma })
        }
        _ => Err(WavyteError::validation(format!(
            "unknown effect kind '{kind}'"
        ))),
    }
}

/// Fold parsed effects into inline and pass-level representations.
pub fn normalize_effects(effects: &[Effect]) -> FxPipeline {
    let mut inline = InlineFx::default();
    let mut passes = Vec::<PassFx>::new();

    for e in effects {
        match *e {
            Effect::OpacityMul { value } => inline.opacity_mul *= value,
            Effect::TransformPost { value } => inline.transform_post *= value,
            Effect::Blur { radius_px, sigma } => match radius_px > 0 {
                true => passes.push(PassFx::Blur { radius_px, sigma }),
                false => {}
            },
        }
    }

    match !inline.opacity_mul.is_finite() || inline.opacity_mul < 0.0 {
        true => inline.opacity_mul = 0.0,
        false => {}
    }

    match inline.opacity_mul == 1.0 && inline.transform_post == Affine::IDENTITY && passes.is_empty()
    {
        true => FxPipeline::default(),
        false => FxPipeline { inline, passes },
    }
}

fn get_u32(obj: &serde_json::Value, key: &str) -> WavyteResult<u32> {
    let v = obj
        .get(key)
        .ok_or_else(|| WavyteError::validation(format!("missing effect param '{key}'")))?;
    let n = v
        .as_u64()
        .ok_or_else(|| WavyteError::validation(format!("effect param '{key}' must be an integer")))?;
    u32::try_from(n)
        .map_err(|_| WavyteError::validation(format!("effect param '{key}' is out of range")))
}

fn get_f32(obj: &serde_json::Value, key: &str) -> WavyteResult<f32> {
    let v = obj
        .get(key)
        .ok_or_else(|| WavyteError::validation(format!("missing effect param '{key}'")))?;
    let n = v
        .as_f64()
        .ok_or_else(|| WavyteError::validation(format!("effect param '{key}' must be a number")))?
        as f32;
    n.is_finite()
        .then_some(n)
        .ok_or_else(|| WavyteError::validation(format!("effect param '{key}' must be finite")))
}

fn parse_pair_f64(
    v: &serde_json::Value,
    shape_err: &'static str,
    first_err: &'static str,
    second_err: &'static str,
) -> WavyteResult<(f64, f64)> {
    let a = v.as_array().ok_or_else(|| WavyteError::validation(shape_err))?;
    (a.len() == 2)
        .then_some(())
        .ok_or_else(|| WavyteError::validation(shape_err))?;
    let first = a[0]
        .as_f64()
        .ok_or_else(|| WavyteError::validation(first_err))?;
    let second = a[1]
        .as_f64()
        .ok_or_else(|| WavyteError::validation(second_err))?;
    Ok((first, second))
}

fn parse_affine(params: &serde_json::Value) -> WavyteResult<Affine> {
    match params.get("affine") {
        Some(a) => {
            let arr = a
                .as_array()
                .ok_or_else(|| WavyteError::validation("transform_post.affine must be an array"))?;
            (arr.len() == 6).then_some(()).ok_or_else(|| {
                WavyteError::validation("transform_post.affine must have length 6")
            })?;
            let mut coeffs = [0.0f64; 6];
            for (i, v) in arr.iter().enumerate() {
                coeffs[i] = v.as_f64().ok_or_else(|| {
                    WavyteError::validation("transform_post.affine entries must be numbers")
                })?;
            }
            Ok(Affine::new(coeffs))
        }
        None => {
            let t = match params.get("translate") {
                Some(v) => {
                    let (x, y) = parse_pair_f64(
                        v,
                        "transform_post.translate must be [x,y]",
                        "transform_post.translate x must be number",
                        "transform_post.translate y must be number",
                    )?;
                    Affine::translate((x, y))
                }
                None => Affine::IDENTITY,
            };

            let rot = match (params.get("rotation_rad"), params.get("rotate_deg")) {
                (Some(v), _) => {
                    let r = v.as_f64().ok_or_else(|| {
                        WavyteError::validation("transform_post.rotation_rad must be number")
                    })?;
                    Affine::rotate(r)
                }
                (None, Some(v)) => {
                    let deg = v.as_f64().ok_or_else(|| {
                        WavyteError::validation("transform_post.rotate_deg must be number")
                    })?;
                    Affine::rotate(deg.to_radians())
                }
                (None, None) => Affine::IDENTITY,
            };

            let scale = match params.get("scale") {
                Some(v) => {
                    let (sx, sy) = parse_pair_f64(
                        v,
                        "transform_post.scale must be [sx,sy]",
                        "transform_post.scale sx must be number",
                        "transform_post.scale sy must be number",
                    )?;
                    Affine::scale_non_uniform(sx, sy)
                }
                None => Affine::IDENTITY,
            };

            Ok(t * rot * scale)
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/effects/fx.rs"]
mod tests;
