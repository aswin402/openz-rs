use crate::ProcessOperation;
use image::{DynamicImage, ImageBuffer};
use openmedia_core::{OpenMediaError, Result};
use wgpu::util::DeviceExt;

pub fn apply_gpu_operation(img: &DynamicImage, op: &ProcessOperation) -> Result<DynamicImage> {
    pollster::block_on(apply_gpu_operation_async(img, op))
}

async fn apply_gpu_operation_async(
    img: &DynamicImage,
    op: &ProcessOperation,
) -> Result<DynamicImage> {
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            ..Default::default()
        })
        .await
        .ok_or_else(|| OpenMediaError::GpuError("No GPU adapter found".to_string()))?;

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                label: Some("OpenMedia GPU Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: Default::default(),
            },
            None,
        )
        .await
        .map_err(|e| OpenMediaError::GpuError(e.to_string()))?;

    match op {
        ProcessOperation::Invert => {
            let width = img.width();
            let height = img.height();
            let rgba = img.to_rgba8();

            // Extract pixels into u32 with native endianness to guarantee alignment
            let raw_pixels: Vec<u32> = rgba
                .chunks_exact(4)
                .map(|c| u32::from_ne_bytes([c[0], c[1], c[2], c[3]]))
                .collect();

            let pixel_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Pixel Buffer"),
                contents: bytemuck::cast_slice(&raw_pixels),
                usage: wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_SRC
                    | wgpu::BufferUsages::COPY_DST,
            });

            // Dimensions: x: width, y: height, z: padding, w: padding (total 16 bytes for vec4 alignment)
            let dimensions_data = [width, height, 0, 0];
            let dim_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Dimensions Buffer"),
                contents: bytemuck::cast_slice(&dimensions_data),
                usage: wgpu::BufferUsages::UNIFORM,
            });

            let shader_src = include_str!("shaders/invert.wgsl");
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Invert Shader"),
                source: wgpu::ShaderSource::Wgsl(shader_src.into()),
            });

            let bind_group_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Bind Group Layout"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: false },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::COMPUTE,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                    ],
                });

            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Bind Group"),
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: pixel_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: dim_buffer.as_entire_binding(),
                    },
                ],
            });

            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Pipeline Layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

            let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("Compute Pipeline"),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            });

            let mut encoder =
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
            {
                let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: None,
                    timestamp_writes: None,
                });
                compute_pass.set_pipeline(&pipeline);
                compute_pass.set_bind_group(0, &bind_group, &[]);
                compute_pass.dispatch_workgroups(width.div_ceil(16), height.div_ceil(16), 1);
            }

            let size = (raw_pixels.len() * 4) as u64;
            let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Output Read Buffer"),
                size,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });

            encoder.copy_buffer_to_buffer(&pixel_buffer, 0, &output_buffer, 0, size);
            queue.submit(Some(encoder.finish()));

            let buffer_slice = output_buffer.slice(..);
            let (tx, rx) = std::sync::mpsc::channel();
            buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.send(result);
            });
            device.poll(wgpu::Maintain::Wait);

            rx.recv()
                .map_err(|e| OpenMediaError::GpuError(e.to_string()))?
                .map_err(|e| OpenMediaError::GpuError(e.to_string()))?;

            let data = buffer_slice.get_mapped_range();
            let result_pixels: &[u32] = bytemuck::cast_slice(&data);

            let mut output_rgba = ImageBuffer::new(width, height);
            {
                let flat_raw = output_rgba.as_mut();
                for (i, &pixel) in result_pixels.iter().enumerate() {
                    let bytes = pixel.to_ne_bytes();
                    flat_raw[i * 4] = bytes[0];
                    flat_raw[i * 4 + 1] = bytes[1];
                    flat_raw[i * 4 + 2] = bytes[2];
                    flat_raw[i * 4 + 3] = bytes[3];
                }
            }

            Ok(DynamicImage::ImageRgba8(output_rgba))
        }
        _ => Err(OpenMediaError::Internal(
            "Operation not supported on GPU".to_string(),
        )),
    }
}
