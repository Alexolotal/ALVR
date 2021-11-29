use alvr_common::glam::UVec2;
use alvr_graphics::TARGET_FORMAT;
use alvr_session::ColorCorrectionDesc;
use wgpu::{
    BindGroup, CommandEncoder, Device, Extent3d, RenderPipeline, TextureDescriptor,
    TextureDimension, TextureUsages, TextureView,
};

pub struct ColorCorrectionPass {
    input: TextureView,
    pipeline: RenderPipeline,
    bind_group: BindGroup,
}

impl ColorCorrectionPass {
    pub fn new(device: &Device, input_size: UVec2) -> Self {
        let texture = device.create_texture(&TextureDescriptor {
            label: None,
            size: Extent3d {
                width: input_size.x,
                height: input_size.y,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TARGET_FORMAT,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::STORAGE_BINDING,
        });

        let input = texture.create_view(&Default::default());

        let pipeline = alvr_graphics::create_default_render_pipeline(
            device,
            include_str!("../resources/color_correction.wgsl"),
        );

        let bind_group = alvr_graphics::create_default_bind_group(device, &pipeline, &input);

        Self {
            input,
            pipeline,
            bind_group,
        }
    }

    pub fn input(&self) -> &TextureView {
        &self.input
    }

    pub fn draw(
        &self,
        encoder: &mut CommandEncoder,
        desc: &ColorCorrectionDesc,
        output: &TextureView,
    ) {
        alvr_graphics::execute_default_pass(
            encoder,
            &self.pipeline,
            &self.bind_group,
            bytemuck::bytes_of(desc),
            output,
        )
    }
}