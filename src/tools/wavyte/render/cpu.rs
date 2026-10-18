use std::collections::{HashMap, VecDeque};

use crate::tools::wavyte::{
    assets::media,
    assets::store::{AssetId, PreparedAsset, PreparedAssetStore},
    assets::svg_raster::{SvgRasterKey, rasterize_svg_to_premul_rgba8, svg_raster_params},
    compile::{CompositeOp, DrawOp, SurfaceDesc, SurfaceId},
    foundation::error::{WavyteError, WavyteResult},
    render::passes::PassBackend,
    render::{FrameRGBA, RenderBackend, RenderSettings},
};

/// CPU renderer implementation backed by `vello_cpu`.
pub struct CpuBackend {
    settings: RenderSettings,
    image_cache: HashMap<AssetId, vello_cpu::Image>,
    svg_cache: HashMap<SvgRasterKey, vello_cpu::Image>,
    font_cache: HashMap<AssetId, vello_cpu::peniko::FontData>,
    video_decoders: HashMap<AssetId, VideoFrameDecoder>,
    surfaces: HashMap<SurfaceId, CpuSurface>,
}

struct CpuSurface {
    width: u16,
    height: u16,
    pixmap: vello_cpu::Pixmap,
}

struct VideoFrameDecoder {
    info: std::sync::Arc<media::VideoSourceInfo>,
    frame_cache: HashMap<u64, vello_cpu::Image>,
    lru: VecDeque<u64>,
    capacity: usize,
    prefetch_frames: u32,
}

impl VideoFrameDecoder {
    fn new(info: std::sync::Arc<media::VideoSourceInfo>) -> Self {
        let capacity = std::env::var("WAVYTE_VIDEO_CACHE_CAPACITY")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|&n| n > 0)
            .unwrap_or(64);
        let prefetch_frames = std::env::var("WAVYTE_VIDEO_PREFETCH_FRAMES")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|&n| n > 0)
            .unwrap_or(12);
        Self {
            info,
            frame_cache: HashMap::new(),
            lru: VecDeque::new(),
            capacity,
            prefetch_frames,
        }
    }

    fn decode_at(&mut self, source_time_s: f64) -> WavyteResult<vello_cpu::Image> {
        let key = self.key_for_time(source_time_s);
        match self.frame_cache.get(&key).cloned() {
            Some(img) => {
                self.touch(key);
                return Ok(img);
            }
            None => {}
        }

        match self.prefetch_for_key(key) {
            Ok(()) => match self.frame_cache.get(&key).cloned() {
                Some(img) => {
                    self.touch(key);
                    return Ok(img);
                }
                None => {}
            },
            Err(_) => {}
        }

        // Fallback for sparse decode requests where batch prefetch didn't include the key.
        let rgba = media::decode_video_frame_rgba8(&self.info, source_time_s)?;
        let image = self.rgba_to_image(&rgba)?;
        self.insert_frame(key, image.clone());
        Ok(image)
    }

    fn key_for_time(&self, source_time_s: f64) -> u64 {
        ((source_time_s.max(0.0)) * 1000.0).round() as u64
    }

    fn prefetch_for_key(&mut self, key_ms: u64) -> WavyteResult<()> {
        let source_fps = self.info.source_fps();
        let step_ms = match source_fps.is_finite() && source_fps > 0.0 {
            true => 1000.0 / source_fps,
            false => 1.0,
        };
        let window_ms = (step_ms * self.prefetch_frames as f64).max(step_ms);
        let bucket = ((key_ms as f64) / window_ms).floor();
        let start_key_ms = (bucket * window_ms).round().max(0.0) as u64;
        let start_time_s = (start_key_ms as f64) / 1000.0;
        let frames =
            media::decode_video_frames_rgba8(&self.info, start_time_s, self.prefetch_frames)?;

        for (offset, rgba) in frames.iter().enumerate() {
            let key = ((start_key_ms as f64) + ((offset as f64) * step_ms)).round() as u64;
            match self.frame_cache.contains_key(&key) {
                true => {
                    self.touch(key);
                }
                false => {
                    let image = self.rgba_to_image(rgba)?;
                    self.insert_frame(key, image);
                }
            }
        }
        Ok(())
    }

    fn rgba_to_image(&self, rgba: &[u8]) -> WavyteResult<vello_cpu::Image> {
        let pixmap = image_premul_bytes_to_pixmap(rgba, self.info.width, self.info.height)?;
        Ok(vello_cpu::Image {
            image: vello_cpu::ImageSource::Pixmap(std::sync::Arc::new(pixmap)),
            sampler: vello_cpu::peniko::ImageSampler::default(),
        })
    }

    fn insert_frame(&mut self, key: u64, image: vello_cpu::Image) {
        self.frame_cache.insert(key, image);
        self.touch(key);
        while self.lru.len() > self.capacity {
            match self.lru.pop_front() {
                Some(old) => {
                    self.frame_cache.remove(&old);
                }
                None => {}
            }
        }
    }

    fn touch(&mut self, key: u64) {
        match self.lru.iter().position(|x| *x == key) {
            Some(pos) => {
                self.lru.remove(pos);
            }
            None => {}
        }
        self.lru.push_back(key);
    }
}

impl CpuBackend {
    /// Construct a CPU backend with the provided render settings.
    pub fn new(settings: RenderSettings) -> Self {
        Self {
            settings,
            image_cache: HashMap::new(),
            svg_cache: HashMap::new(),
            font_cache: HashMap::new(),
            video_decoders: HashMap::new(),
            surfaces: HashMap::new(),
        }
    }

    fn get_composite_surface(&self, id: SurfaceId) -> WavyteResult<&CpuSurface> {
        self.surfaces.get(&id).ok_or_else(|| {
            WavyteError::evaluation(format!(
                "composite src surface {:?} was not initialized",
                id
            ))
        })
    }
}

impl PassBackend for CpuBackend {
    fn ensure_surface(&mut self, id: SurfaceId, desc: &SurfaceDesc) -> WavyteResult<()> {
        let width_u16: u16 = desc
            .width
            .try_into()
            .map_err(|_| WavyteError::evaluation("surface width exceeds u16"))?;
        let height_u16: u16 = desc
            .height
            .try_into()
            .map_err(|_| WavyteError::evaluation("surface height exceeds u16"))?;

        match self.surfaces.get_mut(&id) {
            Some(surface) => match surface.width != width_u16 || surface.height != height_u16 {
                true => {
                    *surface = CpuSurface {
                        width: width_u16,
                        height: height_u16,
                        pixmap: vello_cpu::Pixmap::new(width_u16, height_u16),
                    };
                }
                false => {}
            },
            None => {
                self.surfaces.insert(
                    id,
                    CpuSurface {
                        width: width_u16,
                        height: height_u16,
                        pixmap: vello_cpu::Pixmap::new(width_u16, height_u16),
                    },
                );
            }
        }

        match id == SurfaceId(0) {
            true => {
                let premul = self
                    .settings
                    .clear_rgba
                    .map(|[r, g, b, a]| premul_rgba8(r, g, b, a))
                    .unwrap_or([0, 0, 0, 0]);
                let s = self
                    .surfaces
                    .get_mut(&SurfaceId(0))
                    .ok_or_else(|| WavyteError::evaluation("surface 0 missing"))?;
                clear_pixmap(&mut s.pixmap, premul);
            }
            false => {}
        }
        Ok(())
    }

    fn exec_scene(
        &mut self,
        pass: &crate::tools::wavyte::compile::ScenePass,
        assets: &PreparedAssetStore,
    ) -> WavyteResult<()> {
        let mut surface = self.surfaces.remove(&pass.target).ok_or_else(|| {
            WavyteError::evaluation(format!(
                "scene target surface {:?} was not initialized",
                pass.target
            ))
        })?;

        match pass.clear_to_transparent {
            true => clear_pixmap(&mut surface.pixmap, [0, 0, 0, 0]),
            false => {}
        }

        let mut ctx = vello_cpu::RenderContext::new(surface.width, surface.height);
        for op in &pass.ops {
            draw_op(self, &mut ctx, op, assets)?;
        }
        ctx.flush();
        ctx.render_to_pixmap(&mut surface.pixmap);
        self.surfaces.insert(pass.target, surface);
        Ok(())
    }

    fn exec_offscreen(
        &mut self,
        pass: &crate::tools::wavyte::compile::OffscreenPass,
        _assets: &PreparedAssetStore,
    ) -> WavyteResult<()> {
        let mut output = self.surfaces.remove(&pass.output).ok_or_else(|| {
            WavyteError::evaluation(format!(
                "offscreen output surface {:?} was not initialized",
                pass.output
            ))
        })?;

        let (w, h) = (u32::from(output.width), u32::from(output.height));
        let input_bytes = match pass.input == pass.output {
            true => output.pixmap.data_as_u8_slice().to_vec(),
            false => {
                let input = self.surfaces.get(&pass.input).ok_or_else(|| {
                    WavyteError::evaluation(format!(
                        "offscreen input surface {:?} was not initialized",
                        pass.input
                    ))
                })?;
                match input.width != output.width || input.height != output.height {
                    true => {
                        return Err(WavyteError::evaluation(
                            "offscreen input/output surface size mismatch",
                        ));
                    }
                    false => input.pixmap.data_as_u8_slice().to_vec(),
                }
            }
        };

        match pass.fx {
            crate::tools::wavyte::effects::fx::PassFx::Blur { radius_px, sigma } => {
                let blurred = crate::tools::wavyte::effects::blur::blur_rgba8_premul(
                    &input_bytes,
                    w,
                    h,
                    radius_px,
                    sigma,
                )?;
                output
                    .pixmap
                    .data_as_u8_slice_mut()
                    .copy_from_slice(&blurred);
            }
        }

        self.surfaces.insert(pass.output, output);
        Ok(())
    }

    fn exec_composite(
        &mut self,
        pass: &crate::tools::wavyte::compile::CompositePass,
        _assets: &PreparedAssetStore,
    ) -> WavyteResult<()> {
        let mut dst = self.surfaces.remove(&pass.target).ok_or_else(|| {
            WavyteError::evaluation(format!(
                "composite target surface {:?} was not initialized",
                pass.target
            ))
        })?;

        for op in &pass.ops {
            match *op {
                CompositeOp::Over { src, opacity } => {
                    let src = self.get_composite_surface(src)?;
                    crate::tools::wavyte::effects::composite::over_in_place(
                        dst.pixmap.data_as_u8_slice_mut(),
                        src.pixmap.data_as_u8_slice(),
                        opacity,
                    )?;
                }
                CompositeOp::Crossfade { a, b, t } => {
                    let a = self.get_composite_surface(a)?;
                    let b = self.get_composite_surface(b)?;
                    crate::tools::wavyte::effects::composite::crossfade_over_in_place(
                        dst.pixmap.data_as_u8_slice_mut(),
                        a.pixmap.data_as_u8_slice(),
                        b.pixmap.data_as_u8_slice(),
                        t,
                    )?;
                }
                CompositeOp::Wipe {
                    a,
                    b,
                    t,
                    dir,
                    soft_edge,
                } => {
                    let a = self.get_composite_surface(a)?;
                    let b = self.get_composite_surface(b)?;
                    crate::tools::wavyte::effects::composite::wipe_over_in_place(
                        dst.pixmap.data_as_u8_slice_mut(),
                        a.pixmap.data_as_u8_slice(),
                        b.pixmap.data_as_u8_slice(),
                        crate::tools::wavyte::effects::composite::WipeParams {
                            width: u32::from(dst.width),
                            height: u32::from(dst.height),
                            t,
                            dir,
                            soft_edge,
                        },
                    )?;
                }
            }
        }
        self.surfaces.insert(pass.target, dst);
        Ok(())
    }

    fn readback_rgba8(
        &mut self,
        surface: SurfaceId,
        plan: &crate::tools::wavyte::compile::RenderPlan,
        _assets: &PreparedAssetStore,
    ) -> WavyteResult<FrameRGBA> {
        let s = self.surfaces.get(&surface).ok_or_else(|| {
            WavyteError::evaluation(format!(
                "readback surface {:?} was not initialized",
                surface
            ))
        })?;
        let frame_data = s.pixmap.data_as_u8_slice().to_vec();
        let surface_cap = plan.surfaces.len() as u32;
        self.surfaces.retain(|id, _| id.0 < surface_cap);

        Ok(FrameRGBA {
            width: plan.canvas.width,
            height: plan.canvas.height,
            data: frame_data,
            premultiplied: true,
        })
    }
}

impl RenderBackend for CpuBackend {
    fn worker_render_settings(&self) -> Option<RenderSettings> {
        Some(self.settings.clone())
    }
}

fn premul_rgba8(r: u8, g: u8, b: u8, a: u8) -> [u8; 4] {
    let af = (a as u16) + 1;
    let premul = |c: u8| -> u8 { (((c as u16) * af) >> 8) as u8 };
    [premul(r), premul(g), premul(b), a]
}

fn clear_pixmap(pixmap: &mut vello_cpu::Pixmap, rgba: [u8; 4]) {
    let data = pixmap.data_as_u8_slice_mut();
    for px in data.chunks_exact_mut(4) {
        px.copy_from_slice(&rgba);
    }
}

fn with_opacity_layer(
    ctx: &mut vello_cpu::RenderContext,
    opacity: f32,
    f: impl FnOnce(&mut vello_cpu::RenderContext),
) {
    let use_layer = opacity < 1.0;
    match use_layer {
        true => ctx.push_opacity_layer(opacity),
        false => {}
    }
    f(ctx);
    match use_layer {
        true => ctx.pop_layer(),
        false => {}
    }
}

fn draw_op(
    backend: &mut CpuBackend,
    ctx: &mut vello_cpu::RenderContext,
    op: &DrawOp,
    assets: &PreparedAssetStore,
) -> WavyteResult<()> {
    ctx.set_paint_transform(vello_cpu::kurbo::Affine::IDENTITY);

    match op {
        DrawOp::FillPath {
            path,
            transform,
            color,
            opacity,
            blend: _,
            z: _,
        } => {
            ctx.set_transform(affine_to_cpu(*transform));
            ctx.set_paint(vello_cpu::peniko::Color::from_rgba8(
                color.r, color.g, color.b, color.a,
            ));
            let cpu_path = bezpath_to_cpu(path);
            with_opacity_layer(ctx, *opacity, |ctx| {
                ctx.fill_path(&cpu_path);
            });
            Ok(())
        }
        DrawOp::Image {
            asset,
            transform,
            opacity,
            blend: _,
            z: _,
        } => {
            let image_paint = backend.image_paint_for(*asset, assets)?;
            let (w, h) = image_paint_size(&image_paint)?;

            ctx.set_transform(affine_to_cpu(*transform));
            ctx.set_paint(image_paint);
            with_opacity_layer(ctx, *opacity, |ctx| {
                ctx.fill_rect(&vello_cpu::kurbo::Rect::new(0.0, 0.0, w, h));
            });

            Ok(())
        }
        DrawOp::Text {
            asset,
            transform,
            opacity,
            blend: _,
            z: _,
        } => {
            let prepared = assets.get(*asset)?;
            let t = match prepared {
                PreparedAsset::Text(t) => t,
                _ => return Err(WavyteError::evaluation("AssetId is not a PreparedText")),
            };

            let font = backend.font_for_text_asset(*asset, assets)?;
            ctx.set_transform(affine_to_cpu(*transform));

            with_opacity_layer(ctx, *opacity, |ctx| {
                for line in t.layout.lines() {
                    for item in line.items() {
                        match item {
                            parley::layout::PositionedLayoutItem::GlyphRun(run) => {
                                let brush = run.style().brush;
                                ctx.set_paint(vello_cpu::peniko::Color::from_rgba8(
                                    brush.r, brush.g, brush.b, brush.a,
                                ));

                                let glyphs = run.glyphs().map(|g| vello_cpu::Glyph {
                                    id: g.id,
                                    x: g.x,
                                    y: g.y,
                                });
                                ctx.glyph_run(&font)
                                    .font_size(run.run().font_size())
                                    .fill_glyphs(glyphs);
                            }
                            _ => {}
                        }
                    }
                }
            });

            Ok(())
        }
        DrawOp::Svg {
            asset,
            transform,
            opacity,
            blend: _,
            z: _,
        } => {
            let (svg_paint, w, h, transform_adjust) =
                backend.svg_paint_for(*asset, *transform, assets)?;

            ctx.set_transform(affine_to_cpu(transform_adjust));
            ctx.set_paint(svg_paint);
            with_opacity_layer(ctx, *opacity, |ctx| {
                ctx.fill_rect(&vello_cpu::kurbo::Rect::new(0.0, 0.0, w, h));
            });

            Ok(())
        }
        DrawOp::Video {
            asset,
            source_time_s,
            transform,
            opacity,
            blend: _,
            z: _,
        } => {
            let video_paint = backend.video_paint_for(*asset, *source_time_s, assets)?;
            let (w, h) = image_paint_size(&video_paint)?;

            ctx.set_transform(affine_to_cpu(*transform));
            ctx.set_paint(video_paint);
            with_opacity_layer(ctx, *opacity, |ctx| {
                ctx.fill_rect(&vello_cpu::kurbo::Rect::new(0.0, 0.0, w, h));
            });
            Ok(())
        }
    }
}

fn affine_to_cpu(a: crate::tools::wavyte::foundation::core::Affine) -> vello_cpu::kurbo::Affine {
    vello_cpu::kurbo::Affine::new(a.as_coeffs())
}

fn point_to_cpu(p: crate::tools::wavyte::foundation::core::Point) -> vello_cpu::kurbo::Point {
    vello_cpu::kurbo::Point::new(p.x, p.y)
}

fn bezpath_to_cpu(path: &crate::tools::wavyte::foundation::core::BezPath) -> vello_cpu::kurbo::BezPath {
    use kurbo::PathEl;

    let mut out = vello_cpu::kurbo::BezPath::new();
    for &el in path.elements() {
        match el {
            PathEl::MoveTo(p) => out.move_to(point_to_cpu(p)),
            PathEl::LineTo(p) => out.line_to(point_to_cpu(p)),
            PathEl::QuadTo(p1, p2) => out.quad_to(point_to_cpu(p1), point_to_cpu(p2)),
            PathEl::CurveTo(p1, p2, p3) => {
                out.curve_to(point_to_cpu(p1), point_to_cpu(p2), point_to_cpu(p3));
            }
            PathEl::ClosePath => out.close_path(),
        }
    }
    out
}

fn image_premul_bytes_to_pixmap(
    rgba8_premul: &[u8],
    width: u32,
    height: u32,
) -> WavyteResult<vello_cpu::Pixmap> {
    let w: u16 = width
        .try_into()
        .map_err(|_| WavyteError::evaluation("image width exceeds u16"))?;
    let h: u16 = height
        .try_into()
        .map_err(|_| WavyteError::evaluation("image height exceeds u16"))?;
    match rgba8_premul.len() != width as usize * height as usize * 4 {
        true => {
            return Err(WavyteError::evaluation(
                "prepared image byte length mismatch",
            ));
        }
        false => {}
    }

    let mut may_have_opacities = false;
    let mut pixels = Vec::with_capacity(width as usize * height as usize);
    for px in rgba8_premul.chunks_exact(4) {
        let a = px[3];
        may_have_opacities |= a != 255;
        pixels.push(vello_cpu::peniko::color::PremulRgba8 {
            r: px[0],
            g: px[1],
            b: px[2],
            a,
        });
    }

    Ok(vello_cpu::Pixmap::from_parts_with_opacity(
        pixels,
        w,
        h,
        may_have_opacities,
    ))
}

fn image_paint_size(image: &vello_cpu::Image) -> WavyteResult<(f64, f64)> {
    match &image.image {
        vello_cpu::ImageSource::Pixmap(p) => Ok((f64::from(p.width()), f64::from(p.height()))),
        vello_cpu::ImageSource::OpaqueId(_) => Err(WavyteError::evaluation(
            "cpu backend does not support opaque image ids",
        )),
    }
}

impl CpuBackend {
    fn image_paint_for(
        &mut self,
        id: AssetId,
        assets: &PreparedAssetStore,
    ) -> WavyteResult<vello_cpu::Image> {
        match self.image_cache.get(&id) {
            Some(paint) => return Ok(paint.clone()),
            None => {}
        }

        let prepared = assets.get(id)?;
        let img = match prepared {
            PreparedAsset::Image(img) => img,
            _ => return Err(WavyteError::evaluation("AssetId is not a PreparedImage")),
        };

        let pixmap =
            image_premul_bytes_to_pixmap(img.rgba8_premul.as_slice(), img.width, img.height)?;
        let paint = vello_cpu::Image {
            image: vello_cpu::ImageSource::Pixmap(std::sync::Arc::new(pixmap)),
            sampler: vello_cpu::peniko::ImageSampler::default(),
        };

        self.image_cache.insert(id, paint.clone());
        Ok(paint)
    }

    fn font_for_text_asset(
        &mut self,
        id: AssetId,
        assets: &PreparedAssetStore,
    ) -> WavyteResult<vello_cpu::peniko::FontData> {
        match self.font_cache.get(&id) {
            Some(font) => return Ok(font.clone()),
            None => {}
        }

        let prepared = assets.get(id)?;
        let t = match prepared {
            PreparedAsset::Text(t) => t,
            _ => return Err(WavyteError::evaluation("AssetId is not a PreparedText")),
        };

        let font_bytes = t.font_bytes.as_ref().clone();
        let font = vello_cpu::peniko::FontData::new(vello_cpu::peniko::Blob::from(font_bytes), 0);
        self.font_cache.insert(id, font.clone());
        Ok(font)
    }

    fn svg_paint_for(
        &mut self,
        id: AssetId,
        transform: crate::tools::wavyte::foundation::core::Affine,
        assets: &PreparedAssetStore,
    ) -> WavyteResult<(vello_cpu::Image, f64, f64, crate::tools::wavyte::foundation::core::Affine)> {
        let prepared = assets.get(id)?;
        let svg = match prepared {
            PreparedAsset::Svg(svg) => svg,
            _ => return Err(WavyteError::evaluation("AssetId is not a PreparedSvg")),
        };

        let (w, h, transform_adjust) = svg_raster_params(&svg.tree, transform)?;
        let key = SvgRasterKey {
            asset: id,
            width: w,
            height: h,
        };
        match self.svg_cache.get(&key) {
            Some(paint) => return Ok((paint.clone(), w as f64, h as f64, transform_adjust)),
            None => {}
        }

        let rgba8_premul = rasterize_svg_to_premul_rgba8(&svg.tree, w, h)?;
        let pixmap = image_premul_bytes_to_pixmap(rgba8_premul.as_slice(), w, h)?;

        let paint = vello_cpu::Image {
            image: vello_cpu::ImageSource::Pixmap(std::sync::Arc::new(pixmap)),
            sampler: vello_cpu::peniko::ImageSampler::default(),
        };

        self.svg_cache.insert(key, paint.clone());
        Ok((paint, w as f64, h as f64, transform_adjust))
    }

    fn video_paint_for(
        &mut self,
        id: AssetId,
        source_time_s: f64,
        assets: &PreparedAssetStore,
    ) -> WavyteResult<vello_cpu::Image> {
        let prepared = assets.get(id)?;
        let video = match prepared {
            PreparedAsset::Video(video) => video,
            _ => return Err(WavyteError::evaluation("AssetId is not a PreparedVideo")),
        };
        let decoder = self
            .video_decoders
            .entry(id)
            .or_insert_with(|| VideoFrameDecoder::new(video.info.clone()));
        decoder.decode_at(source_time_s)
    }
}
