use std::{ffi::{c_void, CString}, mem::{align_of, self}, io::Cursor, error::Error};

use ash::{vk::{self, DescriptorSetLayout, DescriptorSet}, util::{Align, read_spv}};

use crate::{interface::Interface, offset_of, octree::{Octree, TreeNode}, uniform::Uniform, Pref};

#[derive(Clone, Debug, Copy)]
struct Vertex {
    pos: [f32; 4],
    uv: [f32; 2],
}

pub struct Pipe {
    pub renderpass: vk::RenderPass,
    pub framebuffer_list: Vec<vk::Framebuffer>,

    pub index_buffer_data: Vec<u32>,
    pub index_buffer: vk::Buffer,
    pub index_buffer_memory: vk::DeviceMemory,
    pub vertex_input_buffer: vk::Buffer,
    pub vertex_input_buffer_memory: vk::DeviceMemory,
    pub uniform_buffer: vk::Buffer,
    pub uniform_buffer_memory: vk::DeviceMemory,

    pub descriptor_pool: vk::DescriptorPool,
    pub desc_set_layout_list: Vec<DescriptorSetLayout>,
    pub descriptor_set_list: Vec<DescriptorSet>,

    pub vertex_code: Vec<u32>,
    pub frag_code: Vec<u32>,
    pub vertex_shader_module: vk::ShaderModule,
    pub fragment_shader_module: vk::ShaderModule,
    pub pipeline_layout: vk::PipelineLayout,

    pub viewport: Vec<vk::Viewport>,
    pub scissor: Vec<vk::Rect2D>,
    pub graphic_pipeline: vk::Pipeline,
}

impl Pipe {
    pub fn init(interface: &Interface, pref: &Pref, ) -> Pipe {
        unsafe {
            log::info!("Creating Renderpass ...");
            let renderpass_attachment = [
                vk::AttachmentDescription { format: interface.surface_format.format, samples: vk::SampleCountFlags::TYPE_1, load_op: vk::AttachmentLoadOp::CLEAR, store_op: vk::AttachmentStoreOp::STORE, final_layout: vk::ImageLayout::PRESENT_SRC_KHR, ..Default::default() },
                vk::AttachmentDescription { format: vk::Format::D16_UNORM, samples: vk::SampleCountFlags::TYPE_1, load_op: vk::AttachmentLoadOp::CLEAR, initial_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL, final_layout: vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL, ..Default::default() },
            ];

            let color_attachment_ref = [vk::AttachmentReference { attachment: 0, layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL, }];
            let depend = [vk::SubpassDependency { src_subpass: vk::SUBPASS_EXTERNAL, src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT, dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_READ | vk::AccessFlags::COLOR_ATTACHMENT_WRITE, dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT, ..Default::default() }];

            let subpass = vk::SubpassDescription::builder()
                .color_attachments(&color_attachment_ref)
                .pipeline_bind_point(vk::PipelineBindPoint::GRAPHICS);

            let renderpass_create_info = vk::RenderPassCreateInfo::builder()
                .attachments(&renderpass_attachment)
                .subpasses(std::slice::from_ref(&subpass))
                .dependencies(&depend);
            
            let renderpass = interface.device
                .create_render_pass(&renderpass_create_info, None, )
                .unwrap();

            log::info!("Getting Framebuffer List ...");
            let framebuffer_list: Vec<vk::Framebuffer> = interface.present_img_view_list
                .iter()
                .map(| &present_image_view | {
                    let framebuffer_attachment = [present_image_view];
                    let frame_buffer_create_info = vk::FramebufferCreateInfo::builder()
                        .render_pass(renderpass)
                        .attachments(&framebuffer_attachment)
                        .width(interface.surface_resolution.width)
                        .height(interface.surface_resolution.height)
                        .layers(1);
                    
                    interface.device
                        .create_framebuffer(&frame_buffer_create_info, None, )
                        .unwrap()
                })
                .collect();

            // Create Index Buffer
            log::info!("Creating IndexBuffer ...");
            let index_buffer_data: Vec<u32> = vec![0u32, 1, 2, 2, 3, 0];
            let index_buffer_info = vk::BufferCreateInfo { size: std::mem::size_of_val(&index_buffer_data) as u64, usage: vk::BufferUsageFlags::INDEX_BUFFER, sharing_mode: vk::SharingMode::EXCLUSIVE, ..Default::default() };

            let index_buffer = interface.device.create_buffer(&index_buffer_info, None).unwrap();
            let index_buffer_memory_req = interface.device.get_buffer_memory_requirements(index_buffer);
            let index_buffer_memory_index = 
                interface.find_memorytype_index(&index_buffer_memory_req, vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT, )
                .expect("ERR_INDEX_BUFFER_MEM_INDEX");
            
            let index_allocate_info = vk::MemoryAllocateInfo { allocation_size: index_buffer_memory_req.size, memory_type_index: index_buffer_memory_index, ..Default::default() };
            let index_buffer_memory = interface.device
                .allocate_memory(&index_allocate_info, None, )
                .unwrap();
            let index_ptr: *mut c_void = interface.device
                .map_memory(index_buffer_memory, 0, index_buffer_memory_req.size, vk::MemoryMapFlags::empty(), )
                .unwrap();
            let mut index_slice = Align::new(index_ptr, align_of::<u32>() as u64, index_buffer_memory_req.size, );

            index_slice.copy_from_slice(&index_buffer_data);
            interface.device.unmap_memory(index_buffer_memory);

            interface.device
                .bind_buffer_memory(index_buffer, index_buffer_memory, 0)
                .unwrap();

            // Create Vertex Buffer
            log::info!("Creating VertexBuffer ...");
            let vertex_list = [
                Vertex { pos: [-1.0, -1.0, 0.0, 1.0], uv: [0.0, 0.0], },
                Vertex { pos: [-1.0, 1.0, 0.0, 1.0], uv: [0.0, 1.0], },
                Vertex { pos: [1.0, 1.0, 0.0, 1.0], uv: [1.0, 1.0], },
                Vertex { pos: [1.0, -1.0, 0.0, 1.0], uv: [1.0, 0.0], },
            ];

            let vertex_input_buffer_info = vk::BufferCreateInfo { size: std::mem::size_of_val(&vertex_list) as u64, usage: vk::BufferUsageFlags::VERTEX_BUFFER, sharing_mode: vk::SharingMode::EXCLUSIVE, ..Default::default() };
            let vertex_input_buffer = interface.device
                .create_buffer(&vertex_input_buffer_info, None, )
                .unwrap();
            let vertex_input_buffer_memory_req = interface.device
                .get_buffer_memory_requirements(vertex_input_buffer);
            let vertex_input_buffer_memory_index = 
                interface.find_memorytype_index(&vertex_input_buffer_memory_req, vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT, )
                .expect("ERR_VERTEX_MEM_INDEX");
            let vertex_buffer_allocate_info = vk::MemoryAllocateInfo { allocation_size: vertex_input_buffer_memory_req.size, memory_type_index: vertex_input_buffer_memory_index, ..Default::default() };
            let vertex_input_buffer_memory = interface.device
                .allocate_memory(&vertex_buffer_allocate_info, None, )
                .unwrap();
            
            let vert_ptr = interface.device
                .map_memory(vertex_input_buffer_memory, 0, vertex_input_buffer_memory_req.size, vk::MemoryMapFlags::empty(), )
                .unwrap();
            let mut slice = Align::new(vert_ptr, align_of::<Vertex>() as u64, vertex_input_buffer_memory_req.size, );
            slice.copy_from_slice(&vertex_list);
            interface.device.unmap_memory(vertex_input_buffer_memory);
            interface.device
                .bind_buffer_memory(vertex_input_buffer, vertex_input_buffer_memory, 0, )
                .unwrap();
            
            // Create Uniform Buffer
            log::info!("Creating UniformBuffer ...");
            let uniform_buffer_data = Uniform::empty();
            let uniform_buffer_info = vk::BufferCreateInfo { size: std::mem::size_of_val(&uniform_buffer_data) as u64, usage: vk::BufferUsageFlags::UNIFORM_BUFFER, sharing_mode: vk::SharingMode::EXCLUSIVE, ..Default::default() };
            
            let uniform_buffer = interface.device
                .create_buffer(&uniform_buffer_info, None, )
                .unwrap();
            let uniform_buffer_memory_req = interface.device
                .get_buffer_memory_requirements(uniform_buffer);
            let uniform_buffer_memory_index = interface.find_memorytype_index(&uniform_buffer_memory_req, vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT, )
                .expect("ERR_UNIFORM_MEM_INDEX");
            let uniform_buffer_allocate_info = vk::MemoryAllocateInfo { allocation_size: uniform_buffer_memory_req.size, memory_type_index: uniform_buffer_memory_index, ..Default::default() };
            let uniform_buffer_memory = interface.device
                .allocate_memory(&uniform_buffer_allocate_info, None, )
                .unwrap();
            let uniform_ptr = interface.device
                .map_memory(uniform_buffer_memory, 0, uniform_buffer_memory_req.size, vk::MemoryMapFlags::empty(), )
                .unwrap();
            let mut uniform_aligned_slice = Align::new(uniform_ptr, align_of::<Uniform>() as u64, uniform_buffer_memory_req.size, );
            uniform_aligned_slice.copy_from_slice(&[uniform_buffer_data]);
            interface.device.unmap_memory(uniform_buffer_memory);
            interface.device
                .bind_buffer_memory(uniform_buffer, uniform_buffer_memory, 0, )
                .unwrap();

            // Create Octree Buffer
            log::info!("Creating OctreeBuffer ...");
            let octree_buffer_data = Octree::collect(0, 1000, 2048.0);
            let octree_buffer_info = vk::BufferCreateInfo { size: std::mem::size_of_val(&octree_buffer_data.data) as u64, usage: vk::BufferUsageFlags::STORAGE_BUFFER, sharing_mode: vk::SharingMode::EXCLUSIVE, ..Default::default() };
            
            let octree_buffer = interface.device
                .create_buffer(&octree_buffer_info, None, )
                .unwrap();
            let octree_buffer_memory_req = interface.device
                .get_buffer_memory_requirements(octree_buffer);
            let octree_buffer_memory_index = interface.find_memorytype_index(&octree_buffer_memory_req, vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT, )
                .expect("ERR_OCTREE_MEM_INDEX");
            let octree_buffer_allocate_info = vk::MemoryAllocateInfo { allocation_size: octree_buffer_memory_req.size, memory_type_index: octree_buffer_memory_index, ..Default::default() };
            let octree_buffer_memory = interface.device
                .allocate_memory(&octree_buffer_allocate_info, None, )
                .unwrap();
            let octree_buffer_ptr = interface.device
                .map_memory(octree_buffer_memory, 0, octree_buffer_memory_req.size, vk::MemoryMapFlags::empty(), )
                .unwrap();
            let mut octree_aligned_slice = Align::new(octree_buffer_ptr, align_of::<TreeNode>() as u64, octree_buffer_memory_req.size, );
            octree_aligned_slice.copy_from_slice(&octree_buffer_data.data);
            interface.device.unmap_memory(octree_buffer_memory);
            interface.device
                .bind_buffer_memory(octree_buffer, octree_buffer_memory, 0, )
                .unwrap();

            // Create DescriptorSet
            log::info!("Creating DescriptorPool ...");
            let descriptor_size_list = [
                vk::DescriptorPoolSize { ty: vk::DescriptorType::UNIFORM_BUFFER, descriptor_count: 1, },
                vk::DescriptorPoolSize { ty: vk::DescriptorType::STORAGE_BUFFER, descriptor_count: 1, },
            ];

            let descriptor_pool_info = vk::DescriptorPoolCreateInfo::builder()
                .pool_sizes(&descriptor_size_list)
                .max_sets(descriptor_size_list.len() as u32);
            let descriptor_pool = interface.device
                .create_descriptor_pool(&descriptor_pool_info, None, )
                .unwrap();
            
            let uniform_set_binding_list = [
                vk::DescriptorSetLayoutBinding { descriptor_type: vk::DescriptorType::UNIFORM_BUFFER, descriptor_count: 1, stage_flags: vk::ShaderStageFlags::FRAGMENT, ..Default::default() },
            ];

            let octree_set_binding_list = [
                vk::DescriptorSetLayoutBinding { descriptor_type: vk::DescriptorType::STORAGE_BUFFER, descriptor_count: 1, stage_flags: vk::ShaderStageFlags::FRAGMENT, ..Default::default() },
            ];

            let uniform_desc_info = vk::DescriptorSetLayoutCreateInfo::builder().bindings(&uniform_set_binding_list);
            let octree_dec_info = vk::DescriptorSetLayoutCreateInfo::builder().bindings(&octree_set_binding_list);

            let desc_set_layout_list: Vec<vk::DescriptorSetLayout> = vec![
                interface.device
                    .create_descriptor_set_layout(&uniform_desc_info, None, )
                    .unwrap(),
                interface.device
                    .create_descriptor_set_layout(&octree_dec_info, None, )
                    .unwrap(),
            ];

            let desc_alloc_info = vk::DescriptorSetAllocateInfo::builder()
                .descriptor_pool(descriptor_pool)
                .set_layouts(&desc_set_layout_list);
            let descriptor_set_list = interface.device
                .allocate_descriptor_sets(&desc_alloc_info)
                .unwrap();

            let uniform_buffer_descriptor = vk::DescriptorBufferInfo { buffer: uniform_buffer, offset: 0, range: mem::size_of_val(&uniform_buffer_data) as u64, };
            let octree_buffer_descriptor = vk::DescriptorBufferInfo { buffer: octree_buffer, offset: 0, range: mem::size_of_val(&octree_buffer_data) as u64, };

            let write_desc_set_list = [
                vk::WriteDescriptorSet { dst_set: descriptor_set_list[0], descriptor_count: 1, descriptor_type: vk::DescriptorType::UNIFORM_BUFFER, p_buffer_info: &uniform_buffer_descriptor, ..Default::default() },
                vk::WriteDescriptorSet { dst_set: descriptor_set_list[1], descriptor_count: 1, descriptor_type: vk::DescriptorType::STORAGE_BUFFER, p_buffer_info: &octree_buffer_descriptor, ..Default::default() },
            ];

            interface.device.update_descriptor_sets(&write_desc_set_list, &[], );

            log::info!("Getting ShaderCode ...");
            let mut vertex_spv_file = Cursor::new(&include_bytes!("../shader/new/vert.spv")[..]);
            let mut frag_spv_file = Cursor::new(&include_bytes!("../shader/new/frag.spv")[..]);

            let vertex_code = read_spv(&mut vertex_spv_file).expect("ERR_READ_VERTEX_SPV");
            let vertex_shader_info = vk::ShaderModuleCreateInfo::builder().code(&vertex_code);

            let frag_code = read_spv(&mut frag_spv_file).expect("ERR_READ_FRAG_SPV");
            let frag_shader_info = vk::ShaderModuleCreateInfo::builder().code(&frag_code);

            let vertex_shader_module = interface.device
                .create_shader_module(&vertex_shader_info, None, )
                .expect("ERR_VERTEX_MODULE");

            let fragment_shader_module = interface.device
                .create_shader_module(&frag_shader_info, None, )
                .expect("ERR_FRAG_MODULE");

            let layout_create_info =
                vk::PipelineLayoutCreateInfo::builder().set_layouts(&desc_set_layout_list);

            log::info!("Creating PipelineLayout ...");
            let pipeline_layout = interface.device
                .create_pipeline_layout(&layout_create_info, None, )
                .unwrap();

            log::info!("Stage Creation ...");
            let shader_entry_name = CString::new("main").unwrap();
            let shader_stage_info_list = [
                vk::PipelineShaderStageCreateInfo { module: vertex_shader_module, p_name: shader_entry_name.as_ptr(), stage: vk::ShaderStageFlags::VERTEX, ..Default::default() },
                vk::PipelineShaderStageCreateInfo { module: fragment_shader_module, p_name: shader_entry_name.as_ptr(), stage: vk::ShaderStageFlags::FRAGMENT, ..Default::default() },
            ];

            let vertex_input_binding_description_list = [
                vk::VertexInputBindingDescription { binding: 0, stride: mem::size_of::<Vertex>() as u32, input_rate: vk::VertexInputRate::VERTEX, }
            ];

            let vertex_input_attribute_description_list = [
                vk::VertexInputAttributeDescription { location: 0, binding: 0, format: vk::Format::R32G32B32A32_SFLOAT, offset: offset_of!(Vertex, pos) as u32, },
                vk::VertexInputAttributeDescription { location: 1, binding: 0, format: vk::Format::R32G32_SFLOAT, offset: offset_of!(Vertex, uv) as u32, },
            ];

            let vertex_input_state_info = vk::PipelineVertexInputStateCreateInfo::builder()
                .vertex_attribute_descriptions(&vertex_input_attribute_description_list)
                .vertex_binding_descriptions(&vertex_input_binding_description_list);

            let vertex_input_assembly_state_info = vk::PipelineInputAssemblyStateCreateInfo { topology: vk::PrimitiveTopology::TRIANGLE_LIST, ..Default::default() };

            log::info!("Viewport and Scissor ...");
            let viewport = vec![
                vk::Viewport { 
                    x: 0.0, y: 0.0,
                    width: interface.surface_resolution.width as f32,
                    height: interface.surface_resolution.height as f32,
                    min_depth: 0.0, max_depth: 1.0,
                }
            ];

            let scissor: Vec<vk::Rect2D> = vec![interface.surface_resolution.into()];
            let viewport_state_info = vk::PipelineViewportStateCreateInfo::builder()
                .scissors(&scissor)
                .viewports(&viewport);

            log::info!("Rasterization ...");
            let rasterization_info = vk::PipelineRasterizationStateCreateInfo { front_face: vk::FrontFace::COUNTER_CLOCKWISE, line_width: 1.0, polygon_mode: vk::PolygonMode::FILL, ..Default::default() };
            let multisample_state_info = vk::PipelineMultisampleStateCreateInfo::builder()
                .rasterization_samples(vk::SampleCountFlags::TYPE_1);
            
            log::info!("Blending ...");
            let color_blend_attachment_state_list = [
                vk::PipelineColorBlendAttachmentState {
                    blend_enable: 0,

                    src_color_blend_factor: vk::BlendFactor::SRC_COLOR,
                    dst_color_blend_factor: vk::BlendFactor::ONE_MINUS_DST_COLOR,

                    color_blend_op: vk::BlendOp::ADD,
                    src_alpha_blend_factor: vk::BlendFactor::ZERO,
                    dst_alpha_blend_factor: vk::BlendFactor::ZERO,
                    alpha_blend_op: vk::BlendOp::ADD,

                    color_write_mask: vk::ColorComponentFlags::RGBA,
                }
            ];

            let color_blend_state = vk::PipelineColorBlendStateCreateInfo::builder()
                .logic_op(vk::LogicOp::CLEAR)
                .attachments(&color_blend_attachment_state_list);

            log::info!("Creating DynamicState ...");
            let dynamic_state = [vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
            let dynamic_state_info =
                vk::PipelineDynamicStateCreateInfo::builder().dynamic_states(&dynamic_state);

            log::info!("Pipe incoming ...");
            let graphic_pipeline_info_list = vk::GraphicsPipelineCreateInfo::builder()
                .stages(&shader_stage_info_list)
                .vertex_input_state(&vertex_input_state_info)
                .input_assembly_state(&vertex_input_assembly_state_info)
                .viewport_state(&viewport_state_info)
                .rasterization_state(&rasterization_info)
                .multisample_state(&multisample_state_info)
                .color_blend_state(&color_blend_state)
                .dynamic_state(&dynamic_state_info)
                .layout(pipeline_layout)
                .render_pass(renderpass)
                .build();

            let graphic_pipeline_list = interface.device
                .create_graphics_pipelines(vk::PipelineCache::null(), &[graphic_pipeline_info_list], None, )
                .unwrap();

            log::info!("Rendering initialisation finished ...");
            Pipe {
                renderpass,
                framebuffer_list,
                index_buffer_data,
                index_buffer,
                index_buffer_memory,
                vertex_input_buffer,
                vertex_input_buffer_memory,
                uniform_buffer,
                uniform_buffer_memory,
                descriptor_pool,
                desc_set_layout_list,
                descriptor_set_list,
                vertex_code,
                frag_code,
                vertex_shader_module,
                fragment_shader_module,
                pipeline_layout,
                viewport,
                scissor,
                graphic_pipeline: graphic_pipeline_list[0],
            }
        }
    }

    pub fn draw(&self, interface: &Interface, ) -> Result<bool, Box<dyn Error>> {
        unsafe {
            let next_image = interface.swapchain_loader
                .acquire_next_image(interface.swapchain, std::u64::MAX, interface.present_complete_semaphore, vk::Fence::null(), );

            let present_index = 
                match next_image {
                    Ok((present_index, _, )) => present_index,
                    Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => { return Ok(true); },
                    Err(error) => panic!("ERROR_AQUIRE_IMAGE -> {}", error, ),
                };

            let clear_value = [vk::ClearValue { color: vk::ClearColorValue { float32: [0.0, 0.0, 0.0, 0.0], }, }];

            // Begin Draw
            let render_pass_begin_info = vk::RenderPassBeginInfo::builder()
                .render_pass(self.renderpass)
                .framebuffer(self.framebuffer_list[present_index as usize])
                .render_area(interface.surface_resolution.into())
                .clear_values(&clear_value);

            interface.device
                .wait_for_fences(&[interface.draw_command_fence], true, std::u64::MAX)
                .expect("DEVICE_LOST");

            interface.device
                .reset_fences(&[interface.draw_command_fence])
                .expect("FENCE_RESET_FAILED");
    
            interface.device
                .reset_command_buffer(interface.draw_command_buffer, vk::CommandBufferResetFlags::RELEASE_RESOURCES, )
                .expect("ERR_RESET_CMD_BUFFER");
    
            let command_buffer_begin_info = vk::CommandBufferBeginInfo::builder()
                .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT);
    
            interface.device
                .begin_command_buffer(interface.draw_command_buffer, &command_buffer_begin_info)
                .expect("ERR_BEGIN_CMD_BUFFER");

            // Pipe Rendering Part
            interface.device
                .cmd_begin_render_pass(interface.draw_command_buffer, &render_pass_begin_info, vk::SubpassContents::INLINE, );
            interface.device
                .cmd_bind_descriptor_sets(interface.draw_command_buffer, vk::PipelineBindPoint::GRAPHICS, self.pipeline_layout, 0, &self.descriptor_set_list[..], &[], );
            interface.device
                .cmd_bind_pipeline(interface.draw_command_buffer, vk::PipelineBindPoint::GRAPHICS, self.graphic_pipeline, );
            interface.device
                .cmd_set_viewport(interface.draw_command_buffer, 0, &self.viewport, );
            interface.device
                .cmd_set_scissor(interface.draw_command_buffer, 0, &self.scissor, );
            interface.device
                .cmd_bind_vertex_buffers(interface.draw_command_buffer, 0, &[self.vertex_input_buffer], &[0], );
            interface.device
                .cmd_bind_index_buffer(interface.draw_command_buffer, self.index_buffer, 0, vk::IndexType::UINT32, );
            interface.device
                .cmd_draw_indexed(interface.draw_command_buffer, self.index_buffer_data.len() as u32, 1, 0, 0, 1, );
            interface.device
                .cmd_end_render_pass(interface.draw_command_buffer);
                
            // End Draw
            interface.device
                .end_command_buffer(interface.draw_command_buffer)
                .expect("ERR_END_CMD_BUFFER");
    
            let command_buffer_list: Vec<vk::CommandBuffer> = vec![interface.draw_command_buffer];
    
            let submit_info = vk::SubmitInfo::builder()
                .wait_semaphores(&[interface.present_complete_semaphore])
                .wait_dst_stage_mask(&[vk::PipelineStageFlags::BOTTOM_OF_PIPE])
                .command_buffers(&command_buffer_list)
                .signal_semaphores(&[interface.rendering_complete_semaphore])
                .build();
    
             interface.device
                .queue_submit(interface.present_queue, &[submit_info], interface.draw_command_fence)
                .expect("QUEUE_SUBMIT_FAILED");

            let present_info = vk::PresentInfoKHR { wait_semaphore_count: 1, p_wait_semaphores: &interface.rendering_complete_semaphore, swapchain_count: 1, p_swapchains: &interface.swapchain, p_image_indices: &present_index, ..Default::default() };

            let present_result = interface.swapchain_loader
                .queue_present(interface.present_queue, &present_info);

            match present_result {
                Ok(is_suboptimal) if is_suboptimal => { return Ok(true); },
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => { return Ok(true); },
                Err(error) => panic!("ERROR_PRESENT_SWAP -> {}", error, ), _ => { },
            }
            
            Ok(false)
        }
    }

    pub fn recreate_swapchain(&mut self, interface: &mut Interface, pref: &Pref, new_extent: vk::Extent2D, ) {
        unsafe {
            interface.wait_for_gpu().expect("DEVICE_LOST");

            log::info!("Recreating Swapchain ...");
            self.framebuffer_list.iter().for_each(| framebuffer | interface.device.destroy_framebuffer(* framebuffer, None, ));
            interface.present_img_view_list.iter().for_each(| view | interface.device.destroy_image_view(* view, None, ));
            interface.swapchain_loader.destroy_swapchain(interface.swapchain, None, );

            interface.surface_capability = interface.surface_loader
                .get_physical_device_surface_capabilities(interface.phy_device, interface.surface, )
                .unwrap();

            interface.surface_resolution = match interface.surface_capability.current_extent.width {
                std::u32::MAX => new_extent,
                _ => interface.surface_capability.current_extent,
            };

            let present_mode = interface.present_mode_list
                .iter()
                .cloned()
                .find(| &mode | mode == pref.pref_present_mode)
                .unwrap_or(vk::PresentModeKHR::FIFO);

            let swapchain_create_info = vk::SwapchainCreateInfoKHR::builder()
                .surface(interface.surface)
                .min_image_count(interface.desired_image_count)
                .image_color_space(interface.surface_format.color_space)
                .image_format(interface.surface_format.format)
                .image_extent(interface.surface_resolution)
                .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
                .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                .pre_transform(interface.pre_transform)
                .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                .present_mode(present_mode)
                .clipped(true)
                .image_array_layers(1);

            interface.swapchain = interface.swapchain_loader
                .create_swapchain(&swapchain_create_info, None, )
                .unwrap();

            log::info!("Load PresentImgList ...");
            interface.present_img_list = interface.swapchain_loader.get_swapchain_images(interface.swapchain).unwrap();
            interface.present_img_view_list = interface.present_img_list
                .iter()
                .map(| &image | {
                    let create_view_info = vk::ImageViewCreateInfo::builder()
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(interface.surface_format.format)
                        .components(vk::ComponentMapping { r: vk::ComponentSwizzle::R, g: vk::ComponentSwizzle::G, b: vk::ComponentSwizzle::B, a: vk::ComponentSwizzle::A, })
                        .subresource_range(vk::ImageSubresourceRange { aspect_mask: vk::ImageAspectFlags::COLOR, base_mip_level: 0, level_count: 1, base_array_layer: 0, layer_count: 1, })
                        .image(image);
                        interface.device.create_image_view(&create_view_info, None, ).unwrap()
                })
                .collect();

            log::info!("Getting Framebuffer List ...");
            self.framebuffer_list = interface.present_img_view_list
                .iter()
                .map(| &present_image_view | {
                    let framebuffer_attachment = [present_image_view];
                    let frame_buffer_create_info = vk::FramebufferCreateInfo::builder()
                        .render_pass(self.renderpass)
                        .attachments(&framebuffer_attachment)
                        .width(interface.surface_resolution.width)
                        .height(interface.surface_resolution.height)
                        .layers(1);
                    
                    interface.device
                        .create_framebuffer(&frame_buffer_create_info, None)
                        .unwrap()
                })
                .collect();

            self.viewport = vec![
                vk::Viewport { 
                    x: 0.0, y: 0.0,
                    width: interface.surface_resolution.width as f32,
                    height: interface.surface_resolution.height as f32,
                    min_depth: 0.0, max_depth: 1.0,
                }
            ];
    
            self.scissor = vec![interface.surface_resolution.into()];
        }
    }
}