use std::{ffi::CString, io::Cursor, mem};

use ash::{util::read_spv, vk::{self, PushConstantRange}, Device};
use cgmath::num_traits::Pow;
use nalgebra_glm::{Vec2, Vec3, Vec4};

use crate::{
    interface::{interface::Interface, surface::SurfaceGroup},
    offset_of,
    pipe::obj::{BASE_CUBE_IDX, BASE_CUBE_UV, BASE_CUBE_VERT},
    tree::{
        octant::Octant,
        octree::{Octree, MAX_DEPTH, MAX_DEPTH_LIMIT, TEXTURE_ALIGN},
    },
    vector::Vector,
    Pref,
};

use super::{descriptor::DescriptorPool, image::ImageTarget};

#[derive(Clone, Debug, Copy)]
pub struct Vertex {
    pub pos: [f32; 4],
    pub pos_on_edge: [f32; 4],
    pub uv: [f32; 2],
    pub loc_idx: u32,
}

#[derive(Clone, Debug, Copy)]
pub struct LocInfo {
    pub parent_list: [u32; MAX_DEPTH_LIMIT],
    pub last_hit_idx: [u32; MAX_DEPTH_LIMIT],
    pub depth: u32,
    pub span: f32,

    padding: [u32; 2],
}

#[derive(Clone)]
pub struct Shader {
    pub code: Vec<u32>,
    pub module: vk::ShaderModule,

    pub stage_info: vk::PipelineShaderStageCreateInfo,
}

#[derive(Clone)]
pub struct Pipe {
    pub pipe_layout: vk::PipelineLayout,

    pub viewport: Vec<vk::Viewport>,
    pub scissor: Vec<vk::Rect2D>,

    pub pipe: vk::Pipeline,
}

#[derive(Clone, Debug, Copy)]
pub struct JFAPush {
    pub px_per_group: Vec2,
}

// "../../shader/comp.spv"
// include_bytes!("../../shader/comp.spv")

impl Pipe {
    pub fn create_layout(&self, descriptor_pool: &DescriptorPool, push_constant_list: &[PushConstantRange], device: &Device) -> Self {
        unsafe {
            let mut result = self.clone();

            let info = vk::PipelineLayoutCreateInfo::builder()
                .set_layouts(&descriptor_pool.layout_list)
                .push_constant_ranges(push_constant_list)
                .build();

            log::info!("Creating PipelineLayout ...");
            result.pipe_layout = device.create_pipeline_layout(&info, None).unwrap();

            result
        }
    }

    pub fn create_comp_pipe(device: &Device, pool: &DescriptorPool, push_constant_list: &[PushConstantRange]) -> Self {
        unsafe {
            let mut result = Self::default();

            log::info!("Getting ShaderCode ...");
            let mut spv = Cursor::new(&include_bytes!("../../shader/JFA.spv")[..]);

            let code = read_spv(&mut spv).expect("ERR_READ_COMP_SPV");
            let shader_info = vk::ShaderModuleCreateInfo::builder().code(&code);

            let shader_module = device
                .create_shader_module(&shader_info, None)
                .expect("ERR_COMP_MODULE");

            log::info!("Stage Creation ...");
            let shader_entry_name = CString::new("main").unwrap();
            let shader_stage = vk::PipelineShaderStageCreateInfo {
                module: shader_module,
                p_name: shader_entry_name.as_ptr(),
                stage: vk::ShaderStageFlags::COMPUTE,
                ..Default::default()
            };

            result = result.create_layout(pool, push_constant_list, device);

            let compute_pipe_info = vk::ComputePipelineCreateInfo::builder()
                .stage(shader_stage)
                .layout(result.pipe_layout)
                .build();

            result.pipe = device
                .create_compute_pipelines(vk::PipelineCache::null(), &[compute_pipe_info], None)
                .expect("ERROR_CREATE_PIPELINE")[0];

            result
        }
    }

    pub fn get_octree_vert_data(
        octree: &Octree,
        img: &mut image::ImageBuffer<image::Rgba<u8>, Vec<u8>>,
    ) -> (Vec<Vertex>, Vec<u32>, Vec<LocInfo>) {
        let mut vertex_data = vec![];
        let mut index_data = vec![];
        let mut loc_data = vec![];

        let (branch_data, pos_info) = octree.get_new_root_info(Vec4::default());
        let mut leaf_data = vec![];
        octree.collect_branch(&branch_data, &pos_info, &mut leaf_data, 6);

        // log::info!("{:#034b}", leaf_data[0].1.node.get_child_bitmask());

        leaf_data
            .iter()
            .enumerate()
            .for_each(|(leaf_idx, (pos_info, loc_branch_data))| {
                let branch_info = loc_branch_data[pos_info.depth_idx()];
                let center = pos_info.local_pos.xyz() * 2.0 + Vec3::ftv(branch_info.span / 2.0);

                let length = TEXTURE_ALIGN.pow(2) as f32 * leaf_idx as f32;
                let base_px = Vec2::new(
                    (length / img.height() as f32).floor() * TEXTURE_ALIGN,
                    length % img.height() as f32,
                );

                octree.write_branch_to_texture(
                    loc_branch_data,
                    pos_info,
                    img,
                    base_px,
                    pos_info.local_pos,
                    TEXTURE_ALIGN,
                    MAX_DEPTH as u32,
                );

                BASE_CUBE_VERT
                    .iter()
                    .enumerate()
                    .for_each(|(vert_idx, coord)| {
                        vertex_data.push(Vertex {
                            pos: [
                                coord.0 * branch_info.span + center.x,
                                coord.1 * branch_info.span + center.y,
                                coord.2 * branch_info.span + center.z,
                                1.0,
                            ],
                            pos_on_edge: [
                                pos_info.local_pos.x,
                                pos_info.local_pos.y,
                                pos_info.local_pos.z,
                                0.0,
                            ],
                            uv: [
                                BASE_CUBE_UV[vert_idx].0 as f32,
                                BASE_CUBE_UV[vert_idx].1 as f32,
                            ],
                            loc_idx: loc_data.len() as u32,
                        });
                    });

                BASE_CUBE_IDX
                    .iter()
                    .for_each(|idx| index_data.push((idx + (leaf_idx as i32) * 24) as u32));

                let mut parent_list = [0; MAX_DEPTH_LIMIT];

                // set to something that is not an actual index to indicate
                // wether there is an active index in use or not
                let mut last_hit_idx = [8; MAX_DEPTH_LIMIT];

                loc_branch_data
                    .iter()
                    .enumerate()
                    .for_each(|(idx, branch_info)| {
                        parent_list[idx] = branch_info.node;
                        last_hit_idx[idx] = branch_info.mask;
                    });

                loc_data.push(LocInfo {
                    parent_list,
                    last_hit_idx,
                    depth: pos_info.depth,
                    span: branch_info.span,

                    ..Default::default()
                });
            });

        (vertex_data, index_data, loc_data)
    }

    pub fn create_graphic_pipe(
        device: &Device,
        surface: &SurfaceGroup,
        pool: &DescriptorPool,
        push_constant_list: &[PushConstantRange],
    ) -> Self {
        unsafe {
            let mut result = Self::default();

            log::info!("Getting ShaderCode ...");
            let mut vert_spv = Cursor::new(&include_bytes!("../../shader/vert.spv")[..]);
            let mut frag_spv = Cursor::new(&include_bytes!("../../shader/tex_frag.spv")[..]);

            let vert_code = read_spv(&mut vert_spv).expect("ERR_READ_VERTEX_SPV");
            let frag_code = read_spv(&mut frag_spv).expect("ERR_READ_FRAG_SPV");

            let vert_shader_info = vk::ShaderModuleCreateInfo::builder()
                .code(&vert_code)
                .build();
            let frag_shader_info = vk::ShaderModuleCreateInfo::builder()
                .code(&frag_code)
                .build();

            let vert_shader_module = device
                .create_shader_module(&vert_shader_info, None)
                .expect("ERR_VERTEX_MODULE");
            let frag_shader_module = device
                .create_shader_module(&frag_shader_info, None)
                .expect("ERR_VERTEX_MODULE");

            log::info!("Stage Creation ...");
            let shader_entry_name = CString::new("main").unwrap();

            let shader_stage_list = vec![
                vk::PipelineShaderStageCreateInfo {
                    module: vert_shader_module,
                    p_name: shader_entry_name.as_ptr(),
                    stage: vk::ShaderStageFlags::VERTEX,
                    ..Default::default()
                },
                vk::PipelineShaderStageCreateInfo {
                    module: frag_shader_module,
                    p_name: shader_entry_name.as_ptr(),
                    stage: vk::ShaderStageFlags::FRAGMENT,
                    ..Default::default()
                },
            ];

            result = result.create_layout(pool, push_constant_list, device);

            let vertex_binding_list = vec![vk::VertexInputBindingDescription {
                binding: 0,
                stride: mem::size_of::<Vertex>() as u32,
                input_rate: vk::VertexInputRate::VERTEX,
            }];

            let vertex_attrib_list = vec![
                vk::VertexInputAttributeDescription {
                    location: 0,
                    binding: 0,
                    format: vk::Format::R32G32B32A32_SFLOAT,
                    offset: offset_of!(Vertex, pos) as u32,
                },
                vk::VertexInputAttributeDescription {
                    location: 1,
                    binding: 0,
                    format: vk::Format::R32G32B32A32_SFLOAT,
                    offset: offset_of!(Vertex, pos_on_edge) as u32,
                },
                vk::VertexInputAttributeDescription {
                    location: 2,
                    binding: 0,
                    format: vk::Format::R32G32_SFLOAT,
                    offset: offset_of!(Vertex, uv) as u32,
                },
                vk::VertexInputAttributeDescription {
                    location: 3,
                    binding: 0,
                    format: vk::Format::R32_UINT,
                    offset: offset_of!(Vertex, loc_idx) as u32,
                },
            ];

            let vertex_state = vk::PipelineVertexInputStateCreateInfo::builder()
                .vertex_attribute_descriptions(&vertex_attrib_list)
                .vertex_binding_descriptions(&vertex_binding_list)
                .build();

            let vertex_assembly_state = vk::PipelineInputAssemblyStateCreateInfo {
                topology: vk::PrimitiveTopology::TRIANGLE_LIST,
                ..Default::default()
            };

            result.viewport = vec![vk::Viewport {
                width: surface.render_res.width as f32,
                height: surface.render_res.height as f32,
                max_depth: 1.0,

                ..Default::default()
            }];

            result.scissor = vec![surface.render_res.into()];
            let viewport_state = vk::PipelineViewportStateCreateInfo::builder()
                .scissors(&result.scissor)
                .viewports(&result.viewport)
                .build();

            log::info!("Rasterization ...");
            let raster_state = vk::PipelineRasterizationStateCreateInfo {
                front_face: vk::FrontFace::COUNTER_CLOCKWISE,
                line_width: 1.0,
                polygon_mode: vk::PolygonMode::FILL,
                ..Default::default()
            };

            log::info!("Multisample state ...");
            let multisample_state = vk::PipelineMultisampleStateCreateInfo::builder()
                .rasterization_samples(vk::SampleCountFlags::TYPE_1)
                .build();

            log::info!("Creating color blending state ...");
            let color_attachment_list = [vk::PipelineColorBlendAttachmentState {
                blend_enable: 0,

                src_color_blend_factor: vk::BlendFactor::SRC_COLOR,
                dst_color_blend_factor: vk::BlendFactor::ONE_MINUS_DST_COLOR,

                color_blend_op: vk::BlendOp::ADD,
                src_alpha_blend_factor: vk::BlendFactor::ZERO,
                dst_alpha_blend_factor: vk::BlendFactor::ZERO,
                alpha_blend_op: vk::BlendOp::ADD,

                color_write_mask: vk::ColorComponentFlags::RGBA,
            }];

            let color_state = vk::PipelineColorBlendStateCreateInfo::builder()
                .logic_op(vk::LogicOp::CLEAR)
                .attachments(&color_attachment_list)
                .build();

            let noop_state = vk::StencilOpState::builder()
                .fail_op(vk::StencilOp::KEEP)
                .pass_op(vk::StencilOp::KEEP)
                .depth_fail_op(vk::StencilOp::KEEP)
                .compare_op(vk::CompareOp::ALWAYS)
                .build();

            let depth_state = vk::PipelineDepthStencilStateCreateInfo::builder()
                .depth_test_enable(true)
                .depth_write_enable(true)
                .depth_compare_op(vk::CompareOp::LESS_OR_EQUAL)
                .front(noop_state)
                .back(noop_state)
                .max_depth_bounds(1.0)
                .build();

            log::info!("Creating DynamicState ...");
            let dynamic_state = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
            let dynamic_state = vk::PipelineDynamicStateCreateInfo::builder()
                .dynamic_states(&dynamic_state)
                .build();

            log::info!("Creating pipeline rendering ...");
            let mut rendering = vk::PipelineRenderingCreateInfoKHR::builder()
                .color_attachment_formats(&[surface.format.format])
                .depth_attachment_format(vk::Format::D16_UNORM)
                .build();

            let graphic_pipe_info = vk::GraphicsPipelineCreateInfo::builder()
                .stages(&shader_stage_list)
                .vertex_input_state(&vertex_state)
                .input_assembly_state(&vertex_assembly_state)
                .viewport_state(&viewport_state)
                .rasterization_state(&raster_state)
                .multisample_state(&multisample_state)
                .color_blend_state(&color_state)
                .depth_stencil_state(&depth_state)
                .dynamic_state(&dynamic_state)
                .layout(result.pipe_layout)
                .push_next(&mut rendering)
                .build();

            result.pipe = device
                .create_graphics_pipelines(vk::PipelineCache::null(), &[graphic_pipe_info], None)
                .expect("ERROR_CREATE_PIPELINE")[0];

            result
        }
    }

    pub fn first_img_barrier(
        &self,
        image: &ImageTarget,
        present_image: vk::Image,
        device: &Device,
        cmd_buffer: vk::CommandBuffer,
    ) {
        unsafe {
            let basic_subresource_range = vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            };

            let comp_write = vk::ImageMemoryBarrier::builder()
                .image(image.img)
                .old_layout(vk::ImageLayout::UNDEFINED)
                .new_layout(vk::ImageLayout::GENERAL)
                .subresource_range(basic_subresource_range.clone())
                .dst_access_mask(vk::AccessFlags::SHADER_WRITE)
                .build();

            let comp_transfer = vk::ImageMemoryBarrier::builder()
                .image(image.img)
                .old_layout(vk::ImageLayout::GENERAL)
                .new_layout(vk::ImageLayout::TRANSFER_SRC_OPTIMAL)
                .subresource_range(basic_subresource_range.clone())
                .src_access_mask(vk::AccessFlags::SHADER_WRITE)
                .dst_access_mask(vk::AccessFlags::TRANSFER_READ)
                .build();

            let swap_transfer = vk::ImageMemoryBarrier::builder()
                .image(present_image)
                .old_layout(vk::ImageLayout::UNDEFINED)
                .new_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .subresource_range(basic_subresource_range.clone())
                .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .build();

            device.cmd_pipeline_barrier(
                cmd_buffer,
                vk::PipelineStageFlags::COMPUTE_SHADER,
                vk::PipelineStageFlags::ALL_COMMANDS,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[comp_write, comp_transfer, swap_transfer],
            )
        }
    }

    /// Function for blitting one image to another image with possibile
    /// scaling implemented. This function is for fast usage
    /// and not for changing the copy setting.

    pub fn copy_image(
        &self,
        device: &Device,
        cmd_buffer: vk::CommandBuffer,
        pref: &Pref,
        src_img: vk::Image,
        dst_img: vk::Image,
        src_res: vk::Extent2D,
        dst_res: vk::Extent2D,
    ) {
        unsafe {
            let src = vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                mip_level: 0,
                base_array_layer: 0,
                layer_count: 1,
            };
            let dst = vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                mip_level: 0,
                base_array_layer: 0,
                layer_count: 1,
            };

            let blit = vk::ImageBlit {
                src_subresource: src,
                src_offsets: [
                    vk::Offset3D { x: 0, y: 0, z: 0 },
                    vk::Offset3D {
                        x: src_res.width as i32,
                        y: src_res.height as i32,
                        z: 1,
                    },
                ],
                dst_subresource: dst,
                dst_offsets: [
                    vk::Offset3D { x: 0, y: 0, z: 0 },
                    vk::Offset3D {
                        x: dst_res.width as i32,
                        y: dst_res.height as i32,
                        z: 1,
                    },
                ],
            };

            device.cmd_blit_image(
                cmd_buffer,
                src_img,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                dst_img,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[blit],
                pref.img_filter,
            );
        }
    }

    pub fn sec_img_barrier(
        &self,
        present_image: vk::Image,
        device: &Device,
        cmd_buffer: vk::CommandBuffer,
    ) {
        unsafe {
            let basic_subresource_range = vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            };

            let swap_present = vk::ImageMemoryBarrier::builder()
                .image(present_image)
                .old_layout(vk::ImageLayout::TRANSFER_DST_OPTIMAL)
                .new_layout(vk::ImageLayout::PRESENT_SRC_KHR)
                .subresource_range(basic_subresource_range.clone())
                .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                .dst_access_mask(vk::AccessFlags::MEMORY_READ)
                .build();

            device.cmd_pipeline_barrier(
                cmd_buffer,
                vk::PipelineStageFlags::ALL_COMMANDS,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::DependencyFlags::empty(),
                &[],
                &[],
                &[swap_present],
            )
        }
    }

    pub fn drop(&self, device: &Device) {
        unsafe {
            device.destroy_pipeline_layout(self.pipe_layout, None);
            device.destroy_pipeline(self.pipe, None);
        }
    }
}

impl Default for Shader {
    fn default() -> Self {
        Self {
            code: Default::default(),
            module: Default::default(),
            stage_info: Default::default(),
        }
    }
}

impl Default for LocInfo {
    fn default() -> Self {
        Self {
            parent_list: Default::default(),
            last_hit_idx: Default::default(),
            depth: Default::default(),
            span: Default::default(),
            padding: Default::default(),
        }
    }
}

impl Default for Pipe {
    fn default() -> Self {
        Self {
            pipe_layout: Default::default(),
            viewport: Default::default(),
            scissor: Default::default(),
            pipe: Default::default(),
        }
    }
}
