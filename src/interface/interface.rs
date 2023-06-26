use crate::{Pref, interface::phydev::PhyDeviceGroup};
use ash::{
    extensions::{
        ext::DebugUtils,
        khr::{Surface, Swapchain},
    },
    vk::{self, SurfaceTransformFlagsKHR},
    Device, Entry, Instance,
};
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};
use std::{
    error::Error,
    ffi::{c_void, CStr, CString},
};
use winit::{event_loop::EventLoop, monitor::MonitorHandle, window::{WindowBuilder, Window}};

pub struct Interface {
    pub entry: Entry,
    pub instance: Instance,
    pub debug_util_loader: DebugUtils,
    pub debug_call_back: vk::DebugUtilsMessengerEXT,

    pub window: winit::window::Window,
    pub monitor_list: Vec<MonitorHandle>,
    pub monitor: MonitorHandle,

    pub surface_loader: Surface,
    pub surface: vk::SurfaceKHR,

    pub phy_device: PhyDeviceGroup,

    pub device: Device,
    pub present_queue: vk::Queue,

    pub surface_format: vk::SurfaceFormatKHR,
    pub pre_transform: SurfaceTransformFlagsKHR,
    pub swap_img_count: u32,

    pub surface_res: vk::Extent2D,
    pub present_mode_list: Vec<vk::PresentModeKHR>,

    pub swapchain_loader: Swapchain,
    pub swapchain: vk::SwapchainKHR,

    pub present_img_list: Vec<vk::Image>,
    pub present_img_view_list: Vec<vk::ImageView>,

    pub pool: vk::CommandPool,
    pub setup_cmd_buffer: vk::CommandBuffer,
    pub draw_cmd_buffer: vk::CommandBuffer,

    pub present_complete: vk::Semaphore,
    pub render_complete: vk::Semaphore,

    pub draw_cmd_fence: vk::Fence,
    pub setup_cmd_fence: vk::Fence,
}

#[macro_export]
macro_rules! offset_of {
    ($base:path, $field:ident) => {{
        #[allow(unused_unsafe)]
        unsafe {
            let base_zeroed: $base = mem::zeroed();
            std::ptr::addr_of!(base_zeroed.$field) as isize
                - std::ptr::addr_of!(base_zeroed) as isize
        }
    }};
}

unsafe extern "system" fn vulkan_debug_callback(flag: vk::DebugUtilsMessageSeverityFlagsEXT, msg_type: vk::DebugUtilsMessageTypeFlagsEXT, callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT, _: *mut c_void) -> vk::Bool32 {
    use vk::DebugUtilsMessageSeverityFlagsEXT as Flag;
    let message = CStr::from_ptr((*callback_data).p_message);

    match flag {
        Flag::VERBOSE => log::info!("[ {:?} ] {}", msg_type, message.to_str().unwrap(),),
        Flag::INFO => log::info!("[ {:?} ] {}", msg_type, message.to_str().unwrap(),),
        Flag::WARNING => log::info!("[ {:?} ] {}", msg_type, message.to_str().unwrap(),),
        _ => log::info!("[ {:?} ] {}", msg_type, message.to_str().unwrap(),),
    }

    return vk::FALSE;
}

impl Interface {
    pub fn init(event_loop: &EventLoop<()>, pref: &Pref) -> Self {
        unsafe {
            log::info!("Creating Window and EventLoop ...");
            let window = WindowBuilder::new()
                .with_title(pref.name.clone())
                .with_inner_size(winit::dpi::LogicalSize::new(
                    f64::from(pref.start_window_size.width),
                    f64::from(pref.start_window_size.height),
                ))
                .build(event_loop)
                .unwrap();

            // Get list of monitor and choose one
            let monitor_list: Vec<MonitorHandle> = event_loop.available_monitors().collect();
            let monitor = monitor_list.first().expect("ERR_NO_MONITOR").clone();
            log::info!("Moniter is [ {} ]", monitor.name().unwrap(),);

            let entry = Entry::load().unwrap();

            log::info!("Creating VulkanInstance ...");
            let name = CString::new(pref.name.clone()).unwrap();
            let engine_name = CString::new(pref.engine_name.clone()).unwrap();

            let mut ext_name_list =
                ash_window::enumerate_required_extensions(window.raw_display_handle())
                    .unwrap()
                    .to_vec();
            ext_name_list.push(DebugUtils::name().as_ptr());

            #[cfg(any(target_os = "macos", target_os = "ios"))]
            {
                ext_names.push(KhrPortabilityEnumerationFn::name().as_ptr());
                ext_names.push(KhrGetPhysicalDeviceProperties2Fn::name().as_ptr());
            }

            let (major, minor) = match entry.try_enumerate_instance_version().unwrap() {
                Some(version) => (
                    vk::api_version_major(version),
                    vk::api_version_minor(version),
                ),
                None => (1, 0),
            };

            log::info!("Vulkan {:?}.{:?} supported ...", major, minor,);

            let app_info = vk::ApplicationInfo::builder()
                .application_name(name.as_c_str())
                .application_version(vk::make_api_version(0, 0, 1, 0))
                .engine_name(engine_name.as_c_str())
                .engine_version(vk::make_api_version(0, 0, 1, 0))
                .api_version(vk::make_api_version(0, major, minor, 0));

            let create_flag = if cfg!(any(target_os = "macos", target_os = "ios",)) {
                vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR
            } else {
                vk::InstanceCreateFlags::default()
            };

            let create_info = vk::InstanceCreateInfo::builder()
                .application_info(&app_info)
                .enabled_extension_names(&ext_name_list)
                .flags(create_flag);

            let instance: Instance = entry
                .create_instance(&create_info, None)
                .expect("ERR_CREATE_INSTANCE");

            // Debug part -> Validation layer stuff
            let debug_info = vk::DebugUtilsMessengerCreateInfoEXT::builder()
                .message_severity(
                    vk::DebugUtilsMessageSeverityFlagsEXT::ERROR
                        | vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                        | vk::DebugUtilsMessageSeverityFlagsEXT::INFO,
                )
                .message_type(
                    vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                        | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                        | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
                )
                .pfn_user_callback(Some(vulkan_debug_callback));

            let debug_util_loader = DebugUtils::new(&entry, &instance);
            let debug_call_back = debug_util_loader
                .create_debug_utils_messenger(&debug_info, None)
                .unwrap();

            let surface = ash_window::create_surface(
                &entry,
                &instance,
                window.raw_display_handle(),
                window.raw_window_handle(),
                None,
            )
            .unwrap();

            let surface_loader = Surface::new(&entry, &instance);

            log::info!("Creating PhyDevice ...");
            let phy_device = PhyDeviceGroup::default()
                .get_phy_device_list(&instance)
                .get_suitable_phy_device(&instance, &surface_loader, surface)
                .get_phy_device_prop(&instance);

            let device_extension_list = [
                Swapchain::name().as_ptr(),
                // DynamicRendering::name().as_ptr(),
                #[cfg(any(target_os = "macos", target_os = "ios",))]
                KhrPortabilitySubsetFn::name().as_ptr(),
            ];

            let feature = vk::PhysicalDeviceFeatures {
                shader_clip_distance: 1,
                ..Default::default()
            };
            let priority = [1.0];

            log::info!("Get QueueList ...");
            let queue_info = vk::DeviceQueueCreateInfo::builder()
                .queue_family_index(phy_device.queue_family_index)
                .queue_priorities(&priority);

            let mut dynamic_rendering_feature =
                vk::PhysicalDeviceDynamicRenderingFeaturesKHR::builder().dynamic_rendering(true);

            let device_create_info = vk::DeviceCreateInfo::builder()
                .queue_create_infos(std::slice::from_ref(&queue_info))
                .enabled_extension_names(&device_extension_list)
                .enabled_features(&feature)
                .push_next(&mut dynamic_rendering_feature);

            let device: Device = instance
                .create_device(phy_device.device, &device_create_info, None)
                .unwrap();

            let present_queue = device.get_device_queue(phy_device.queue_family_index, 0);

            log::info!("Load Surface ...");
            let surface_format = surface_loader
                .get_physical_device_surface_formats(phy_device.device, surface)
                .unwrap()[0];

            let surface_capa = surface_loader
                .get_physical_device_surface_capabilities(phy_device.device, surface)
                .unwrap();

            let mut swap_img_count = surface_capa.min_image_count + 1;
            if surface_capa.max_image_count > 0
                && swap_img_count > surface_capa.max_image_count
            {
                swap_img_count = surface_capa.max_image_count;
            }

            let (surface_res, _) = Self::get_res(&window, pref, &surface_capa);

            let pre_transform = if surface_capa
                .supported_transforms
                .contains(vk::SurfaceTransformFlagsKHR::IDENTITY)
            {
                vk::SurfaceTransformFlagsKHR::IDENTITY
            } else {
                surface_capa.current_transform
            };

            let present_mode_list = surface_loader
                .get_physical_device_surface_present_modes(phy_device.device, surface)
                .unwrap();

            let present_mode = present_mode_list
                .iter()
                .cloned()
                .find(|&mode| mode == pref.pref_present_mode)
                .unwrap_or(vk::PresentModeKHR::FIFO);

            log::info!("Creating Swapchain ...");
            let swapchain_loader = Swapchain::new(&instance, &device);

            let swapchain_create_info = vk::SwapchainCreateInfoKHR::builder()
                .surface(surface)
                .min_image_count(swap_img_count)
                .image_color_space(surface_format.color_space)
                .image_format(surface_format.format)
                .image_extent(surface_res)
                .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
                .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
                .pre_transform(pre_transform)
                .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
                .present_mode(present_mode)
                .clipped(true)
                .image_array_layers(1);

            let swapchain = swapchain_loader
                .create_swapchain(&swapchain_create_info, None)
                .unwrap();

            log::info!("Creating CommandPool ...");
            let pool_create_info = vk::CommandPoolCreateInfo::builder()
                .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
                .queue_family_index(phy_device.queue_family_index);

            let pool = device.create_command_pool(&pool_create_info, None).unwrap();

            log::info!("Creating CommandBuffer ...");
            let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::builder()
                .command_buffer_count(2)
                .command_pool(pool)
                .level(vk::CommandBufferLevel::PRIMARY);

            let command_buffer_list = device
                .allocate_command_buffers(&command_buffer_allocate_info)
                .unwrap();

            let setup_cmd_buffer = command_buffer_list[0];
            let draw_cmd_buffer = command_buffer_list[1];

            log::info!("Load PresentImgList ...");
            let present_img_list = swapchain_loader.get_swapchain_images(swapchain).unwrap();
            let present_img_view_list: Vec<vk::ImageView> = present_img_list
                .iter()
                .map(|&image| {
                    let create_view_info = vk::ImageViewCreateInfo::builder()
                        .view_type(vk::ImageViewType::TYPE_2D)
                        .format(surface_format.format)
                        .components(vk::ComponentMapping {
                            r: vk::ComponentSwizzle::R,
                            g: vk::ComponentSwizzle::G,
                            b: vk::ComponentSwizzle::B,
                            a: vk::ComponentSwizzle::A,
                        })
                        .subresource_range(vk::ImageSubresourceRange {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            base_mip_level: 0,
                            level_count: 1,
                            base_array_layer: 0,
                            layer_count: 1,
                        })
                        .image(image);
                    device.create_image_view(&create_view_info, None).unwrap()
                })
                .collect();

            log::info!("Init Fence ...");
            let fence_create_info =
                vk::FenceCreateInfo::builder().flags(vk::FenceCreateFlags::SIGNALED);

            let draw_cmd_fence = device
                .create_fence(&fence_create_info, None)
                .expect("FENCE_CREATE_ERR");
            let setup_cmd_fence = device
                .create_fence(&fence_create_info, None)
                .expect("FENCE_CREATE_ERR");

            log::info!("Init Semaphore ...");
            let semaphore_create_info = vk::SemaphoreCreateInfo::default();

            let present_complete = device
                .create_semaphore(&semaphore_create_info, None)
                .unwrap();
            let render_complete = device
                .create_semaphore(&semaphore_create_info, None)
                .unwrap();

            log::info!("Interface finished ...");
            Interface {
                entry,
                instance,

                debug_util_loader,
                debug_call_back,

                window,
                monitor_list,
                monitor,

                surface_loader,
                surface,

                phy_device,

                device,
                present_queue,

                surface_format: surface_format,
                pre_transform,
                swap_img_count,

                surface_res,
                present_mode_list,

                swapchain_loader,
                swapchain,

                present_img_list,
                present_img_view_list,

                pool,
                setup_cmd_buffer,
                draw_cmd_buffer,

                present_complete,
                render_complete,

                draw_cmd_fence,
                setup_cmd_fence,
            }
        }
    }

    pub fn swap_draw_next<Function: FnOnce(u32)>(
        &self,
        function: Function,
    ) -> Result<bool, Box<dyn Error>> {
        unsafe {
            let next_image = self.swapchain_loader.acquire_next_image(
                self.swapchain,
                std::u64::MAX,
                self.present_complete,
                vk::Fence::null(),
            );

            let present_index = match next_image {
                Ok((present_index, _)) => present_index,
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    return Ok(true);
                }
                Err(error) => panic!("ERROR_AQUIRE_IMAGE -> {}", error,),
            };

            function(present_index);

            let present_info = vk::PresentInfoKHR {
                wait_semaphore_count: 1,
                p_wait_semaphores: &self.render_complete,
                swapchain_count: 1,
                p_swapchains: &self.swapchain,
                p_image_indices: &present_index,
                ..Default::default()
            };

            let present_result = self
                .swapchain_loader
                .queue_present(self.present_queue, &present_info);

            match present_result {
                Ok(is_suboptimal) if is_suboptimal => {
                    return Ok(true);
                }
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    return Ok(true);
                }
                Err(error) => panic!("ERROR_PRESENT_SWAP -> {}", error,),
                _ => {}
            }

            Ok(false)
        }
    }

    /// Function to get the resolution
    /// at which to render. The resolution or scale factor
    /// can be changed in pref.
    /// First return is surface and other one is render res.

    pub fn get_res(window: &Window, pref: &Pref, surface_capa: &vk::SurfaceCapabilitiesKHR) -> (vk::Extent2D, vk::Extent2D) {
        // Select new Dimension
        let dim = window.inner_size();
        let surface_res = match surface_capa.current_extent.width {
            std::u32::MAX => vk::Extent2D {
                width: dim.width,
                height: dim.height,
            },
            _ => surface_capa.current_extent,
        };

        // Select new RenderResolution
        let render_res = if pref.use_render_res && window.fullscreen() != None {
            pref.render_res
        } else {
            vk::Extent2D {
                width: (surface_res.width as f32 / pref.img_scale) as u32,
                height: (surface_res.height as f32 / pref.img_scale) as u32,
            }
        };

        (surface_res, render_res)
    }

    pub fn find_memorytype_index(&self, memory_req: &vk::MemoryRequirements, flag: vk::MemoryPropertyFlags) -> Option<u32> {
        self.phy_device.mem_prop.memory_types[..self.phy_device.mem_prop.memory_type_count as _]
            .iter()
            .enumerate()
            .find(|(index, memory_type)| {
                (1 << index) & memory_req.memory_type_bits != 0
                    && memory_type.property_flags & flag == flag
            })
            .map(|(index, _memory_type)| index as _)
    }

    pub fn wait_for_gpu(&self) -> Result<(), Box<dyn Error>> {
        unsafe { Ok(self.device.device_wait_idle().unwrap()) }
    }
}