#![allow(clippy::missing_safety_doc)]

mod config;
mod layer_defs;
mod planner;

use ash::vk;
use ash::vk::Handle;
use config::Mode;
use layer_defs::{
    VkLayerDeviceCreateInfo, VkLayerFunction, VkLayerInstanceCreateInfo, VkNegotiateLayerInterface,
    VkNegotiateLayerStructType, CURRENT_LOADER_LAYER_INTERFACE_VERSION,
};
use planner::{mark_injection_result, mutate_swapchain, planned_sequence};
use std::collections::HashMap;
use std::ffi::{c_char, CStr};
use std::fs::OpenOptions;
use std::io::{Cursor, Write};
use std::mem;
use std::ptr;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

const LAYER_NAME_BYTES: &[u8] = b"VK_LAYER_PPFG_rust\0";
const LAYER_DESCRIPTION_BYTES: &[u8] = b"Post-process frame generation Rust Vulkan layer\0";
const BLEND_VERT_SPV: &[u8] = include_bytes!("../shaders/blend.vert.spv");
const BLEND_FRAG_SPV: &[u8] = include_bytes!("../shaders/blend.frag.spv");

fn layer_name() -> &'static CStr {
    unsafe { CStr::from_bytes_with_nul_unchecked(LAYER_NAME_BYTES) }
}

fn layer_description() -> &'static CStr {
    unsafe { CStr::from_bytes_with_nul_unchecked(LAYER_DESCRIPTION_BYTES) }
}

macro_rules! cstr {
    ($lit:literal) => {
        concat!($lit, "\0").as_ptr().cast::<c_char>()
    };
}

type InstanceCreateInfo = vk::InstanceCreateInfo<'static>;
type DeviceCreateInfo = vk::DeviceCreateInfo<'static>;
type AllocationCallbacks = vk::AllocationCallbacks<'static>;
type DeviceQueueInfo2 = vk::DeviceQueueInfo2<'static>;
type SwapchainCreateInfoKHR = vk::SwapchainCreateInfoKHR<'static>;
type PresentInfoKHR = vk::PresentInfoKHR<'static>;
type SubmitInfo = vk::SubmitInfo<'static>;

type PfnVkGetInstanceProcAddr =
    unsafe extern "system" fn(vk::Instance, *const c_char) -> vk::PFN_vkVoidFunction;
type PfnVkGetDeviceProcAddr =
    unsafe extern "system" fn(vk::Device, *const c_char) -> vk::PFN_vkVoidFunction;
type PfnVkCreateInstance = unsafe extern "system" fn(
    *const InstanceCreateInfo,
    *const AllocationCallbacks,
    *mut vk::Instance,
) -> vk::Result;
type PfnVkDestroyInstance = unsafe extern "system" fn(vk::Instance, *const AllocationCallbacks);
type PfnVkCreateDevice = unsafe extern "system" fn(
    vk::PhysicalDevice,
    *const DeviceCreateInfo,
    *const AllocationCallbacks,
    *mut vk::Device,
) -> vk::Result;
type PfnVkDestroyDevice = unsafe extern "system" fn(vk::Device, *const AllocationCallbacks);
type PfnVkEnumerateDeviceExtensionProperties = unsafe extern "system" fn(
    vk::PhysicalDevice,
    *const c_char,
    *mut u32,
    *mut vk::ExtensionProperties,
) -> vk::Result;
type PfnVkGetPhysicalDeviceProperties =
    unsafe extern "system" fn(vk::PhysicalDevice, *mut vk::PhysicalDeviceProperties);
type PfnVkGetPhysicalDeviceQueueFamilyProperties =
    unsafe extern "system" fn(vk::PhysicalDevice, *mut u32, *mut vk::QueueFamilyProperties);
type PfnVkGetPhysicalDeviceMemoryProperties =
    unsafe extern "system" fn(vk::PhysicalDevice, *mut vk::PhysicalDeviceMemoryProperties);
type PfnVkGetPhysicalDeviceSurfaceCapabilitiesKHR = unsafe extern "system" fn(
    vk::PhysicalDevice,
    vk::SurfaceKHR,
    *mut vk::SurfaceCapabilitiesKHR,
) -> vk::Result;
type PfnVkGetDeviceQueue = unsafe extern "system" fn(vk::Device, u32, u32, *mut vk::Queue);
type PfnVkGetDeviceQueue2 =
    unsafe extern "system" fn(vk::Device, *const DeviceQueueInfo2, *mut vk::Queue);
type PfnVkQueuePresentKHR =
    unsafe extern "system" fn(vk::Queue, *const PresentInfoKHR) -> vk::Result;
type PfnVkCreateSwapchainKHR = unsafe extern "system" fn(
    vk::Device,
    *const SwapchainCreateInfoKHR,
    *const AllocationCallbacks,
    *mut vk::SwapchainKHR,
) -> vk::Result;
type PfnVkDestroySwapchainKHR =
    unsafe extern "system" fn(vk::Device, vk::SwapchainKHR, *const AllocationCallbacks);
type PfnVkGetSwapchainImagesKHR =
    unsafe extern "system" fn(vk::Device, vk::SwapchainKHR, *mut u32, *mut vk::Image) -> vk::Result;
type PfnVkAcquireNextImageKHR = unsafe extern "system" fn(
    vk::Device,
    vk::SwapchainKHR,
    u64,
    vk::Semaphore,
    vk::Fence,
    *mut u32,
) -> vk::Result;
type PfnVkCreateCommandPool = unsafe extern "system" fn(
    vk::Device,
    *const vk::CommandPoolCreateInfo<'static>,
    *const AllocationCallbacks,
    *mut vk::CommandPool,
) -> vk::Result;
type PfnVkDestroyCommandPool =
    unsafe extern "system" fn(vk::Device, vk::CommandPool, *const AllocationCallbacks);
type PfnVkResetCommandPool =
    unsafe extern "system" fn(vk::Device, vk::CommandPool, vk::CommandPoolResetFlags) -> vk::Result;
type PfnVkAllocateCommandBuffers = unsafe extern "system" fn(
    vk::Device,
    *const vk::CommandBufferAllocateInfo<'static>,
    *mut vk::CommandBuffer,
) -> vk::Result;
type PfnVkFreeCommandBuffers =
    unsafe extern "system" fn(vk::Device, vk::CommandPool, u32, *const vk::CommandBuffer);
type PfnVkBeginCommandBuffer = unsafe extern "system" fn(
    vk::CommandBuffer,
    *const vk::CommandBufferBeginInfo<'static>,
) -> vk::Result;
type PfnVkEndCommandBuffer = unsafe extern "system" fn(vk::CommandBuffer) -> vk::Result;
type PfnVkQueueSubmit =
    unsafe extern "system" fn(vk::Queue, u32, *const SubmitInfo, vk::Fence) -> vk::Result;
type PfnVkCreateFence = unsafe extern "system" fn(
    vk::Device,
    *const vk::FenceCreateInfo<'static>,
    *const AllocationCallbacks,
    *mut vk::Fence,
) -> vk::Result;
type PfnVkDestroyFence =
    unsafe extern "system" fn(vk::Device, vk::Fence, *const AllocationCallbacks);
type PfnVkWaitForFences =
    unsafe extern "system" fn(vk::Device, u32, *const vk::Fence, vk::Bool32, u64) -> vk::Result;
type PfnVkResetFences = unsafe extern "system" fn(vk::Device, u32, *const vk::Fence) -> vk::Result;
type PfnVkCreateSemaphore = unsafe extern "system" fn(
    vk::Device,
    *const vk::SemaphoreCreateInfo<'static>,
    *const AllocationCallbacks,
    *mut vk::Semaphore,
) -> vk::Result;
type PfnVkDestroySemaphore =
    unsafe extern "system" fn(vk::Device, vk::Semaphore, *const AllocationCallbacks);
type PfnVkQueueWaitIdle = unsafe extern "system" fn(vk::Queue) -> vk::Result;
type PfnVkCreateImage = unsafe extern "system" fn(
    vk::Device,
    *const vk::ImageCreateInfo<'static>,
    *const AllocationCallbacks,
    *mut vk::Image,
) -> vk::Result;
type PfnVkDestroyImage =
    unsafe extern "system" fn(vk::Device, vk::Image, *const AllocationCallbacks);
type PfnVkGetImageMemoryRequirements =
    unsafe extern "system" fn(vk::Device, vk::Image, *mut vk::MemoryRequirements);
type PfnVkAllocateMemory = unsafe extern "system" fn(
    vk::Device,
    *const vk::MemoryAllocateInfo<'static>,
    *const AllocationCallbacks,
    *mut vk::DeviceMemory,
) -> vk::Result;
type PfnVkFreeMemory =
    unsafe extern "system" fn(vk::Device, vk::DeviceMemory, *const AllocationCallbacks);
type PfnVkBindImageMemory = unsafe extern "system" fn(
    vk::Device,
    vk::Image,
    vk::DeviceMemory,
    vk::DeviceSize,
) -> vk::Result;
type PfnVkCmdPipelineBarrier = unsafe extern "system" fn(
    vk::CommandBuffer,
    vk::PipelineStageFlags,
    vk::PipelineStageFlags,
    vk::DependencyFlags,
    u32,
    *const vk::MemoryBarrier<'static>,
    u32,
    *const vk::BufferMemoryBarrier<'static>,
    u32,
    *const vk::ImageMemoryBarrier<'static>,
);
type PfnVkCmdClearColorImage = unsafe extern "system" fn(
    vk::CommandBuffer,
    vk::Image,
    vk::ImageLayout,
    *const vk::ClearColorValue,
    u32,
    *const vk::ImageSubresourceRange,
);
type PfnVkCmdCopyImage = unsafe extern "system" fn(
    vk::CommandBuffer,
    vk::Image,
    vk::ImageLayout,
    vk::Image,
    vk::ImageLayout,
    u32,
    *const vk::ImageCopy,
);
type PfnVkDeviceWaitIdle = unsafe extern "system" fn(vk::Device) -> vk::Result;
type PfnVkCreateImageView = unsafe extern "system" fn(
    vk::Device,
    *const vk::ImageViewCreateInfo<'static>,
    *const AllocationCallbacks,
    *mut vk::ImageView,
) -> vk::Result;
type PfnVkDestroyImageView =
    unsafe extern "system" fn(vk::Device, vk::ImageView, *const AllocationCallbacks);
type PfnVkCreateSampler = unsafe extern "system" fn(
    vk::Device,
    *const vk::SamplerCreateInfo<'static>,
    *const AllocationCallbacks,
    *mut vk::Sampler,
) -> vk::Result;
type PfnVkDestroySampler =
    unsafe extern "system" fn(vk::Device, vk::Sampler, *const AllocationCallbacks);
type PfnVkCreateShaderModule = unsafe extern "system" fn(
    vk::Device,
    *const vk::ShaderModuleCreateInfo<'static>,
    *const AllocationCallbacks,
    *mut vk::ShaderModule,
) -> vk::Result;
type PfnVkDestroyShaderModule =
    unsafe extern "system" fn(vk::Device, vk::ShaderModule, *const AllocationCallbacks);
type PfnVkCreateDescriptorSetLayout = unsafe extern "system" fn(
    vk::Device,
    *const vk::DescriptorSetLayoutCreateInfo<'static>,
    *const AllocationCallbacks,
    *mut vk::DescriptorSetLayout,
) -> vk::Result;
type PfnVkDestroyDescriptorSetLayout =
    unsafe extern "system" fn(vk::Device, vk::DescriptorSetLayout, *const AllocationCallbacks);
type PfnVkCreateDescriptorPool = unsafe extern "system" fn(
    vk::Device,
    *const vk::DescriptorPoolCreateInfo<'static>,
    *const AllocationCallbacks,
    *mut vk::DescriptorPool,
) -> vk::Result;
type PfnVkDestroyDescriptorPool =
    unsafe extern "system" fn(vk::Device, vk::DescriptorPool, *const AllocationCallbacks);
type PfnVkAllocateDescriptorSets = unsafe extern "system" fn(
    vk::Device,
    *const vk::DescriptorSetAllocateInfo<'static>,
    *mut vk::DescriptorSet,
) -> vk::Result;
type PfnVkUpdateDescriptorSets = unsafe extern "system" fn(
    vk::Device,
    u32,
    *const vk::WriteDescriptorSet<'static>,
    u32,
    *const vk::CopyDescriptorSet<'static>,
);
type PfnVkCreatePipelineLayout = unsafe extern "system" fn(
    vk::Device,
    *const vk::PipelineLayoutCreateInfo<'static>,
    *const AllocationCallbacks,
    *mut vk::PipelineLayout,
) -> vk::Result;
type PfnVkDestroyPipelineLayout =
    unsafe extern "system" fn(vk::Device, vk::PipelineLayout, *const AllocationCallbacks);
type PfnVkCreateRenderPass = unsafe extern "system" fn(
    vk::Device,
    *const vk::RenderPassCreateInfo<'static>,
    *const AllocationCallbacks,
    *mut vk::RenderPass,
) -> vk::Result;
type PfnVkDestroyRenderPass =
    unsafe extern "system" fn(vk::Device, vk::RenderPass, *const AllocationCallbacks);
type PfnVkCreateFramebuffer = unsafe extern "system" fn(
    vk::Device,
    *const vk::FramebufferCreateInfo<'static>,
    *const AllocationCallbacks,
    *mut vk::Framebuffer,
) -> vk::Result;
type PfnVkDestroyFramebuffer =
    unsafe extern "system" fn(vk::Device, vk::Framebuffer, *const AllocationCallbacks);
type PfnVkCreateGraphicsPipelines = unsafe extern "system" fn(
    vk::Device,
    vk::PipelineCache,
    u32,
    *const vk::GraphicsPipelineCreateInfo<'static>,
    *const AllocationCallbacks,
    *mut vk::Pipeline,
) -> vk::Result;
type PfnVkDestroyPipeline =
    unsafe extern "system" fn(vk::Device, vk::Pipeline, *const AllocationCallbacks);
type PfnVkCmdBeginRenderPass = unsafe extern "system" fn(
    vk::CommandBuffer,
    *const vk::RenderPassBeginInfo<'static>,
    vk::SubpassContents,
);
type PfnVkCmdEndRenderPass = unsafe extern "system" fn(vk::CommandBuffer);
type PfnVkCmdBindPipeline =
    unsafe extern "system" fn(vk::CommandBuffer, vk::PipelineBindPoint, vk::Pipeline);
type PfnVkCmdBindDescriptorSets = unsafe extern "system" fn(
    vk::CommandBuffer,
    vk::PipelineBindPoint,
    vk::PipelineLayout,
    u32,
    u32,
    *const vk::DescriptorSet,
    u32,
    *const u32,
);
type PfnVkCmdDraw = unsafe extern "system" fn(vk::CommandBuffer, u32, u32, u32, u32);

#[derive(Default)]
struct LoggerSink {
    file: Option<std::fs::File>,
}

static LOGGER: OnceLock<Mutex<LoggerSink>> = OnceLock::new();

fn logger() -> &'static Mutex<LoggerSink> {
    LOGGER.get_or_init(|| {
        let file = std::env::var("PPFG_LAYER_LOG_FILE")
            .ok()
            .filter(|path| !path.is_empty())
            .and_then(|path| OpenOptions::new().create(true).append(true).open(path).ok());
        Mutex::new(LoggerSink { file })
    })
}

fn now_epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn log(level: &str, message: impl AsRef<str>) {
    let line = format!("[ppfg][{level}][{}] {}\n", now_epoch_ms(), message.as_ref());
    let mut guard = logger().lock().expect("logger mutex poisoned");
    if let Some(file) = guard.file.as_mut() {
        let _ = file.write_all(line.as_bytes());
        let _ = file.flush();
    } else {
        let _ = std::io::stderr().write_all(line.as_bytes());
        let _ = std::io::stderr().flush();
    }
}

fn log_info(message: impl AsRef<str>) {
    log("info", message);
}
fn log_warn(message: impl AsRef<str>) {
    log("warn", message);
}
fn log_error(message: impl AsRef<str>) {
    log("error", message);
}

#[derive(Clone, Copy, Default)]
struct InstanceDispatch {
    get_instance_proc_addr: Option<PfnVkGetInstanceProcAddr>,
    destroy_instance: Option<PfnVkDestroyInstance>,
    create_device: Option<PfnVkCreateDevice>,
    enumerate_device_extension_properties: Option<PfnVkEnumerateDeviceExtensionProperties>,
    get_physical_device_properties: Option<PfnVkGetPhysicalDeviceProperties>,
    get_physical_device_queue_family_properties:
        Option<PfnVkGetPhysicalDeviceQueueFamilyProperties>,
    get_physical_device_memory_properties: Option<PfnVkGetPhysicalDeviceMemoryProperties>,
    get_physical_device_surface_capabilities_khr:
        Option<PfnVkGetPhysicalDeviceSurfaceCapabilitiesKHR>,
}

#[derive(Clone, Copy, Default)]
struct DeviceDispatch {
    get_device_proc_addr: Option<PfnVkGetDeviceProcAddr>,
    destroy_device: Option<PfnVkDestroyDevice>,
    get_device_queue: Option<PfnVkGetDeviceQueue>,
    get_device_queue2: Option<PfnVkGetDeviceQueue2>,
    queue_present_khr: Option<PfnVkQueuePresentKHR>,
    create_swapchain_khr: Option<PfnVkCreateSwapchainKHR>,
    destroy_swapchain_khr: Option<PfnVkDestroySwapchainKHR>,
    get_swapchain_images_khr: Option<PfnVkGetSwapchainImagesKHR>,
    acquire_next_image_khr: Option<PfnVkAcquireNextImageKHR>,
    create_command_pool: Option<PfnVkCreateCommandPool>,
    destroy_command_pool: Option<PfnVkDestroyCommandPool>,
    reset_command_pool: Option<PfnVkResetCommandPool>,
    allocate_command_buffers: Option<PfnVkAllocateCommandBuffers>,
    free_command_buffers: Option<PfnVkFreeCommandBuffers>,
    begin_command_buffer: Option<PfnVkBeginCommandBuffer>,
    end_command_buffer: Option<PfnVkEndCommandBuffer>,
    queue_submit: Option<PfnVkQueueSubmit>,
    create_fence: Option<PfnVkCreateFence>,
    destroy_fence: Option<PfnVkDestroyFence>,
    wait_for_fences: Option<PfnVkWaitForFences>,
    reset_fences: Option<PfnVkResetFences>,
    create_semaphore: Option<PfnVkCreateSemaphore>,
    destroy_semaphore: Option<PfnVkDestroySemaphore>,
    queue_wait_idle: Option<PfnVkQueueWaitIdle>,
    create_image: Option<PfnVkCreateImage>,
    destroy_image: Option<PfnVkDestroyImage>,
    get_image_memory_requirements: Option<PfnVkGetImageMemoryRequirements>,
    allocate_memory: Option<PfnVkAllocateMemory>,
    free_memory: Option<PfnVkFreeMemory>,
    bind_image_memory: Option<PfnVkBindImageMemory>,
    cmd_pipeline_barrier: Option<PfnVkCmdPipelineBarrier>,
    cmd_clear_color_image: Option<PfnVkCmdClearColorImage>,
    cmd_copy_image: Option<PfnVkCmdCopyImage>,
    device_wait_idle: Option<PfnVkDeviceWaitIdle>,
    create_image_view: Option<PfnVkCreateImageView>,
    destroy_image_view: Option<PfnVkDestroyImageView>,
    create_sampler: Option<PfnVkCreateSampler>,
    destroy_sampler: Option<PfnVkDestroySampler>,
    create_shader_module: Option<PfnVkCreateShaderModule>,
    destroy_shader_module: Option<PfnVkDestroyShaderModule>,
    create_descriptor_set_layout: Option<PfnVkCreateDescriptorSetLayout>,
    destroy_descriptor_set_layout: Option<PfnVkDestroyDescriptorSetLayout>,
    create_descriptor_pool: Option<PfnVkCreateDescriptorPool>,
    destroy_descriptor_pool: Option<PfnVkDestroyDescriptorPool>,
    allocate_descriptor_sets: Option<PfnVkAllocateDescriptorSets>,
    update_descriptor_sets: Option<PfnVkUpdateDescriptorSets>,
    create_pipeline_layout: Option<PfnVkCreatePipelineLayout>,
    destroy_pipeline_layout: Option<PfnVkDestroyPipelineLayout>,
    create_render_pass: Option<PfnVkCreateRenderPass>,
    destroy_render_pass: Option<PfnVkDestroyRenderPass>,
    create_framebuffer: Option<PfnVkCreateFramebuffer>,
    destroy_framebuffer: Option<PfnVkDestroyFramebuffer>,
    create_graphics_pipelines: Option<PfnVkCreateGraphicsPipelines>,
    destroy_pipeline: Option<PfnVkDestroyPipeline>,
    cmd_begin_render_pass: Option<PfnVkCmdBeginRenderPass>,
    cmd_end_render_pass: Option<PfnVkCmdEndRenderPass>,
    cmd_bind_pipeline: Option<PfnVkCmdBindPipeline>,
    cmd_bind_descriptor_sets: Option<PfnVkCmdBindDescriptorSets>,
    cmd_draw: Option<PfnVkCmdDraw>,
}

#[derive(Clone, Copy, Default)]
struct QueueInfo {
    device: vk::Device,
    family_index: u32,
    queue_index: u32,
    supports_graphics: bool,
    supports_transfer: bool,
}

#[derive(Clone, Copy, Default)]
struct InjectResources {
    initialized: bool,
    family_index: u32,
    command_pool: vk::CommandPool,
    command_buffer: vk::CommandBuffer,
    acquire_semaphore: vk::Semaphore,
    ready_original_semaphore: vk::Semaphore,
    ready_generated_semaphore: vk::Semaphore,
    submit_fence: vk::Fence,
}

#[derive(Clone, Copy, Default)]
struct BlendResources {
    initialized: bool,
    render_pass: vk::RenderPass,
    pipeline_layout: vk::PipelineLayout,
    pipeline: vk::Pipeline,
    descriptor_set_layout: vk::DescriptorSetLayout,
    descriptor_pool: vk::DescriptorPool,
    descriptor_set: vk::DescriptorSet,
    sampler: vk::Sampler,
    history_view: vk::ImageView,
}

#[derive(Clone)]
struct SwapchainState {
    device: vk::Device,
    physical_device: vk::PhysicalDevice,
    surface: vk::SurfaceKHR,
    handle: vk::SwapchainKHR,
    format: vk::Format,
    extent: vk::Extent2D,
    present_mode: vk::PresentModeKHR,
    original_usage: vk::ImageUsageFlags,
    modified_usage: vk::ImageUsageFlags,
    original_min_image_count: u32,
    modified_min_image_count: u32,
    images: Vec<vk::Image>,
    history_image: vk::Image,
    history_memory: vk::DeviceMemory,
    history_valid: bool,
    present_count: u64,
    generated_present_count: u64,
    injection_attempted: bool,
    injection_works: bool,
    inject: InjectResources,
    blend: BlendResources,
}

impl Default for SwapchainState {
    fn default() -> Self {
        Self {
            device: vk::Device::null(),
            physical_device: vk::PhysicalDevice::null(),
            surface: vk::SurfaceKHR::null(),
            handle: vk::SwapchainKHR::null(),
            format: vk::Format::UNDEFINED,
            extent: vk::Extent2D::default(),
            present_mode: vk::PresentModeKHR::FIFO,
            original_usage: vk::ImageUsageFlags::empty(),
            modified_usage: vk::ImageUsageFlags::empty(),
            original_min_image_count: 0,
            modified_min_image_count: 0,
            images: Vec::new(),
            history_image: vk::Image::null(),
            history_memory: vk::DeviceMemory::null(),
            history_valid: false,
            present_count: 0,
            generated_present_count: 0,
            injection_attempted: false,
            injection_works: false,
            inject: InjectResources::default(),
            blend: BlendResources::default(),
        }
    }
}

#[derive(Clone, Copy, Default)]
struct DeviceInfo {
    instance: vk::Instance,
    physical_device: vk::PhysicalDevice,
    device: vk::Device,
    instance_dispatch: InstanceDispatch,
    dispatch: DeviceDispatch,
}

#[derive(Default)]
struct GlobalState {
    instance_dispatch: HashMap<usize, InstanceDispatch>,
    instance_map: HashMap<usize, vk::Instance>,
    device_map: HashMap<usize, DeviceInfo>,
    queue_map: HashMap<usize, QueueInfo>,
    swapchains: HashMap<u64, SwapchainState>,
}

static GLOBAL_STATE: OnceLock<Mutex<GlobalState>> = OnceLock::new();

fn global_state() -> &'static Mutex<GlobalState> {
    GLOBAL_STATE.get_or_init(|| Mutex::new(GlobalState::default()))
}

unsafe fn dispatch_key_from_handle<T: Handle>(handle: T) -> usize {
    let raw = handle.as_raw();
    if raw == 0 {
        return 0;
    }
    let ptr = raw as *const usize;
    ptr.read()
}

fn queue_id(queue: vk::Queue) -> usize {
    queue.as_raw() as usize
}

unsafe fn cast_pfn<T: Copy>(func: vk::PFN_vkVoidFunction) -> T {
    mem::transmute_copy(&func)
}

unsafe fn load_instance_fn<T: Copy>(
    gipa: PfnVkGetInstanceProcAddr,
    instance: vk::Instance,
    name: *const c_char,
) -> T {
    cast_pfn::<T>(gipa(instance, name))
}

unsafe fn load_device_fn<T: Copy>(
    gdpa: PfnVkGetDeviceProcAddr,
    device: vk::Device,
    name: *const c_char,
) -> T {
    cast_pfn::<T>(gdpa(device, name))
}

unsafe fn fill_instance_dispatch(
    instance: vk::Instance,
    gipa: PfnVkGetInstanceProcAddr,
) -> InstanceDispatch {
    InstanceDispatch {
        get_instance_proc_addr: Some(gipa),
        destroy_instance: load_instance_fn(gipa, instance, cstr!("vkDestroyInstance")),
        create_device: load_instance_fn(gipa, instance, cstr!("vkCreateDevice")),
        enumerate_device_extension_properties: load_instance_fn(
            gipa,
            instance,
            cstr!("vkEnumerateDeviceExtensionProperties"),
        ),
        get_physical_device_properties: load_instance_fn(
            gipa,
            instance,
            cstr!("vkGetPhysicalDeviceProperties"),
        ),
        get_physical_device_queue_family_properties: load_instance_fn(
            gipa,
            instance,
            cstr!("vkGetPhysicalDeviceQueueFamilyProperties"),
        ),
        get_physical_device_memory_properties: load_instance_fn(
            gipa,
            instance,
            cstr!("vkGetPhysicalDeviceMemoryProperties"),
        ),
        get_physical_device_surface_capabilities_khr: load_instance_fn(
            gipa,
            instance,
            cstr!("vkGetPhysicalDeviceSurfaceCapabilitiesKHR"),
        ),
    }
}

unsafe fn fill_device_dispatch(device: vk::Device, gdpa: PfnVkGetDeviceProcAddr) -> DeviceDispatch {
    DeviceDispatch {
        get_device_proc_addr: Some(gdpa),
        destroy_device: load_device_fn(gdpa, device, cstr!("vkDestroyDevice")),
        get_device_queue: load_device_fn(gdpa, device, cstr!("vkGetDeviceQueue")),
        get_device_queue2: load_device_fn(gdpa, device, cstr!("vkGetDeviceQueue2")),
        queue_present_khr: load_device_fn(gdpa, device, cstr!("vkQueuePresentKHR")),
        create_swapchain_khr: load_device_fn(gdpa, device, cstr!("vkCreateSwapchainKHR")),
        destroy_swapchain_khr: load_device_fn(gdpa, device, cstr!("vkDestroySwapchainKHR")),
        get_swapchain_images_khr: load_device_fn(gdpa, device, cstr!("vkGetSwapchainImagesKHR")),
        acquire_next_image_khr: load_device_fn(gdpa, device, cstr!("vkAcquireNextImageKHR")),
        create_command_pool: load_device_fn(gdpa, device, cstr!("vkCreateCommandPool")),
        destroy_command_pool: load_device_fn(gdpa, device, cstr!("vkDestroyCommandPool")),
        reset_command_pool: load_device_fn(gdpa, device, cstr!("vkResetCommandPool")),
        allocate_command_buffers: load_device_fn(gdpa, device, cstr!("vkAllocateCommandBuffers")),
        free_command_buffers: load_device_fn(gdpa, device, cstr!("vkFreeCommandBuffers")),
        begin_command_buffer: load_device_fn(gdpa, device, cstr!("vkBeginCommandBuffer")),
        end_command_buffer: load_device_fn(gdpa, device, cstr!("vkEndCommandBuffer")),
        queue_submit: load_device_fn(gdpa, device, cstr!("vkQueueSubmit")),
        create_fence: load_device_fn(gdpa, device, cstr!("vkCreateFence")),
        destroy_fence: load_device_fn(gdpa, device, cstr!("vkDestroyFence")),
        wait_for_fences: load_device_fn(gdpa, device, cstr!("vkWaitForFences")),
        reset_fences: load_device_fn(gdpa, device, cstr!("vkResetFences")),
        create_semaphore: load_device_fn(gdpa, device, cstr!("vkCreateSemaphore")),
        destroy_semaphore: load_device_fn(gdpa, device, cstr!("vkDestroySemaphore")),
        queue_wait_idle: load_device_fn(gdpa, device, cstr!("vkQueueWaitIdle")),
        create_image: load_device_fn(gdpa, device, cstr!("vkCreateImage")),
        destroy_image: load_device_fn(gdpa, device, cstr!("vkDestroyImage")),
        get_image_memory_requirements: load_device_fn(
            gdpa,
            device,
            cstr!("vkGetImageMemoryRequirements"),
        ),
        allocate_memory: load_device_fn(gdpa, device, cstr!("vkAllocateMemory")),
        free_memory: load_device_fn(gdpa, device, cstr!("vkFreeMemory")),
        bind_image_memory: load_device_fn(gdpa, device, cstr!("vkBindImageMemory")),
        cmd_pipeline_barrier: load_device_fn(gdpa, device, cstr!("vkCmdPipelineBarrier")),
        cmd_clear_color_image: load_device_fn(gdpa, device, cstr!("vkCmdClearColorImage")),
        cmd_copy_image: load_device_fn(gdpa, device, cstr!("vkCmdCopyImage")),
        device_wait_idle: load_device_fn(gdpa, device, cstr!("vkDeviceWaitIdle")),
        create_image_view: load_device_fn(gdpa, device, cstr!("vkCreateImageView")),
        destroy_image_view: load_device_fn(gdpa, device, cstr!("vkDestroyImageView")),
        create_sampler: load_device_fn(gdpa, device, cstr!("vkCreateSampler")),
        destroy_sampler: load_device_fn(gdpa, device, cstr!("vkDestroySampler")),
        create_shader_module: load_device_fn(gdpa, device, cstr!("vkCreateShaderModule")),
        destroy_shader_module: load_device_fn(gdpa, device, cstr!("vkDestroyShaderModule")),
        create_descriptor_set_layout: load_device_fn(
            gdpa,
            device,
            cstr!("vkCreateDescriptorSetLayout"),
        ),
        destroy_descriptor_set_layout: load_device_fn(
            gdpa,
            device,
            cstr!("vkDestroyDescriptorSetLayout"),
        ),
        create_descriptor_pool: load_device_fn(gdpa, device, cstr!("vkCreateDescriptorPool")),
        destroy_descriptor_pool: load_device_fn(gdpa, device, cstr!("vkDestroyDescriptorPool")),
        allocate_descriptor_sets: load_device_fn(gdpa, device, cstr!("vkAllocateDescriptorSets")),
        update_descriptor_sets: load_device_fn(gdpa, device, cstr!("vkUpdateDescriptorSets")),
        create_pipeline_layout: load_device_fn(gdpa, device, cstr!("vkCreatePipelineLayout")),
        destroy_pipeline_layout: load_device_fn(gdpa, device, cstr!("vkDestroyPipelineLayout")),
        create_render_pass: load_device_fn(gdpa, device, cstr!("vkCreateRenderPass")),
        destroy_render_pass: load_device_fn(gdpa, device, cstr!("vkDestroyRenderPass")),
        create_framebuffer: load_device_fn(gdpa, device, cstr!("vkCreateFramebuffer")),
        destroy_framebuffer: load_device_fn(gdpa, device, cstr!("vkDestroyFramebuffer")),
        create_graphics_pipelines: load_device_fn(gdpa, device, cstr!("vkCreateGraphicsPipelines")),
        destroy_pipeline: load_device_fn(gdpa, device, cstr!("vkDestroyPipeline")),
        cmd_begin_render_pass: load_device_fn(gdpa, device, cstr!("vkCmdBeginRenderPass")),
        cmd_end_render_pass: load_device_fn(gdpa, device, cstr!("vkCmdEndRenderPass")),
        cmd_bind_pipeline: load_device_fn(gdpa, device, cstr!("vkCmdBindPipeline")),
        cmd_bind_descriptor_sets: load_device_fn(gdpa, device, cstr!("vkCmdBindDescriptorSets")),
        cmd_draw: load_device_fn(gdpa, device, cstr!("vkCmdDraw")),
    }
}

fn format_hex(value: u64) -> String {
    format!("0x{value:x}")
}

fn usage_flags(flags: vk::ImageUsageFlags) -> String {
    let names = [
        (vk::ImageUsageFlags::TRANSFER_SRC, "TRANSFER_SRC"),
        (vk::ImageUsageFlags::TRANSFER_DST, "TRANSFER_DST"),
        (vk::ImageUsageFlags::SAMPLED, "SAMPLED"),
        (vk::ImageUsageFlags::STORAGE, "STORAGE"),
        (vk::ImageUsageFlags::COLOR_ATTACHMENT, "COLOR_ATTACHMENT"),
        (
            vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
            "DEPTH_STENCIL_ATTACHMENT",
        ),
        (
            vk::ImageUsageFlags::TRANSIENT_ATTACHMENT,
            "TRANSIENT_ATTACHMENT",
        ),
        (vk::ImageUsageFlags::INPUT_ATTACHMENT, "INPUT_ATTACHMENT"),
    ];

    let mut parts = Vec::new();
    for (bit, name) in names {
        if flags.contains(bit) {
            parts.push(name);
        }
    }
    if parts.is_empty() {
        parts.push("0");
    }
    format!(
        "{} ({})",
        parts.join("|"),
        format_hex(flags.as_raw() as u64)
    )
}

fn format_extent(extent: vk::Extent2D) -> String {
    format!("{}x{}", extent.width, extent.height)
}

fn present_mode_name(mode: vk::PresentModeKHR) -> String {
    match mode {
        vk::PresentModeKHR::IMMEDIATE => "IMMEDIATE".into(),
        vk::PresentModeKHR::MAILBOX => "MAILBOX".into(),
        vk::PresentModeKHR::FIFO => "FIFO".into(),
        vk::PresentModeKHR::FIFO_RELAXED => "FIFO_RELAXED".into(),
        _ => format!("{}", mode.as_raw()),
    }
}

fn cstr_opt(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        None
    } else {
        unsafe { Some(CStr::from_ptr(ptr).to_string_lossy().into_owned()) }
    }
}

fn remember_queue(
    queue: vk::Queue,
    device: vk::Device,
    family_index: u32,
    queue_index: u32,
    supports_graphics: bool,
    supports_transfer: bool,
) {
    if queue.is_null() {
        return;
    }

    let info = QueueInfo {
        device,
        family_index,
        queue_index,
        supports_graphics,
        supports_transfer,
    };

    let mut state = global_state().lock().expect("global state mutex poisoned");
    state.queue_map.insert(queue_id(queue), info);
}

unsafe fn destroy_inject_resources(
    dispatch: &DeviceDispatch,
    device: vk::Device,
    inject: &mut InjectResources,
) {
    if !inject.initialized {
        return;
    }

    if let Some(device_wait_idle) = dispatch.device_wait_idle {
        let _ = device_wait_idle(device);
    }
    if let Some(destroy_semaphore) = dispatch.destroy_semaphore {
        if !inject.acquire_semaphore.is_null() {
            destroy_semaphore(device, inject.acquire_semaphore, ptr::null());
            inject.acquire_semaphore = vk::Semaphore::null();
        }
        if !inject.ready_original_semaphore.is_null() {
            destroy_semaphore(device, inject.ready_original_semaphore, ptr::null());
            inject.ready_original_semaphore = vk::Semaphore::null();
        }
        if !inject.ready_generated_semaphore.is_null() {
            destroy_semaphore(device, inject.ready_generated_semaphore, ptr::null());
            inject.ready_generated_semaphore = vk::Semaphore::null();
        }
    }
    if let Some(destroy_fence) = dispatch.destroy_fence {
        if !inject.submit_fence.is_null() {
            destroy_fence(device, inject.submit_fence, ptr::null());
            inject.submit_fence = vk::Fence::null();
        }
    }
    if let (Some(free_command_buffers), Some(destroy_command_pool)) =
        (dispatch.free_command_buffers, dispatch.destroy_command_pool)
    {
        if !inject.command_pool.is_null() {
            if !inject.command_buffer.is_null() {
                free_command_buffers(device, inject.command_pool, 1, &inject.command_buffer);
                inject.command_buffer = vk::CommandBuffer::null();
            }
            destroy_command_pool(device, inject.command_pool, ptr::null());
            inject.command_pool = vk::CommandPool::null();
        }
    }
    inject.initialized = false;
}

unsafe fn find_memory_type_index(
    device_info: &DeviceInfo,
    memory_type_bits: u32,
    required_flags: vk::MemoryPropertyFlags,
) -> Option<u32> {
    let get_memory_properties = device_info
        .instance_dispatch
        .get_physical_device_memory_properties?;
    if device_info.physical_device.is_null() {
        return None;
    }

    let mut memory_properties = vk::PhysicalDeviceMemoryProperties::default();
    get_memory_properties(device_info.physical_device, &mut memory_properties);
    for index in 0..memory_properties.memory_type_count {
        if (memory_type_bits & (1 << index)) == 0 {
            continue;
        }
        let flags = memory_properties.memory_types[index as usize].property_flags;
        if flags.contains(required_flags) {
            return Some(index);
        }
    }
    None
}

unsafe fn ensure_history_image(swapchain: &mut SwapchainState, device_info: &DeviceInfo) -> bool {
    if !swapchain.history_image.is_null() {
        return true;
    }

    let (
        Some(create_image),
        Some(get_image_memory_requirements),
        Some(allocate_memory),
        Some(bind_image_memory),
    ) = (
        device_info.dispatch.create_image,
        device_info.dispatch.get_image_memory_requirements,
        device_info.dispatch.allocate_memory,
        device_info.dispatch.bind_image_memory,
    )
    else {
        log_warn("history image creation functions unavailable");
        return false;
    };

    let image_info = vk::ImageCreateInfo {
        s_type: vk::StructureType::IMAGE_CREATE_INFO,
        image_type: vk::ImageType::TYPE_2D,
        format: swapchain.format,
        extent: vk::Extent3D {
            width: swapchain.extent.width,
            height: swapchain.extent.height,
            depth: 1,
        },
        mip_levels: 1,
        array_layers: 1,
        samples: vk::SampleCountFlags::TYPE_1,
        tiling: vk::ImageTiling::OPTIMAL,
        usage: vk::ImageUsageFlags::TRANSFER_SRC
            | vk::ImageUsageFlags::TRANSFER_DST
            | vk::ImageUsageFlags::SAMPLED,
        sharing_mode: vk::SharingMode::EXCLUSIVE,
        initial_layout: vk::ImageLayout::UNDEFINED,
        ..Default::default()
    };

    if create_image(
        device_info.device,
        &image_info,
        ptr::null(),
        &mut swapchain.history_image,
    ) != vk::Result::SUCCESS
    {
        log_warn("CreateImage failed for history image");
        swapchain.history_image = vk::Image::null();
        return false;
    }

    let mut memory_requirements = vk::MemoryRequirements::default();
    get_image_memory_requirements(
        device_info.device,
        swapchain.history_image,
        &mut memory_requirements,
    );
    let Some(memory_type_index) = find_memory_type_index(
        device_info,
        memory_requirements.memory_type_bits,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    ) else {
        log_warn("failed to find device-local memory type for history image");
        if let Some(destroy_image) = device_info.dispatch.destroy_image {
            destroy_image(device_info.device, swapchain.history_image, ptr::null());
        }
        swapchain.history_image = vk::Image::null();
        return false;
    };

    let alloc_info = vk::MemoryAllocateInfo {
        s_type: vk::StructureType::MEMORY_ALLOCATE_INFO,
        allocation_size: memory_requirements.size,
        memory_type_index,
        ..Default::default()
    };
    if allocate_memory(
        device_info.device,
        &alloc_info,
        ptr::null(),
        &mut swapchain.history_memory,
    ) != vk::Result::SUCCESS
    {
        log_warn("AllocateMemory failed for history image");
        if let Some(destroy_image) = device_info.dispatch.destroy_image {
            destroy_image(device_info.device, swapchain.history_image, ptr::null());
        }
        swapchain.history_image = vk::Image::null();
        swapchain.history_memory = vk::DeviceMemory::null();
        return false;
    }

    if bind_image_memory(
        device_info.device,
        swapchain.history_image,
        swapchain.history_memory,
        0,
    ) != vk::Result::SUCCESS
    {
        log_warn("BindImageMemory failed for history image");
        if let Some(free_memory) = device_info.dispatch.free_memory {
            free_memory(device_info.device, swapchain.history_memory, ptr::null());
        }
        if let Some(destroy_image) = device_info.dispatch.destroy_image {
            destroy_image(device_info.device, swapchain.history_image, ptr::null());
        }
        swapchain.history_memory = vk::DeviceMemory::null();
        swapchain.history_image = vk::Image::null();
        return false;
    }

    swapchain.history_valid = false;
    log_info("created history image for swapchain");
    true
}

unsafe fn create_shader_module_from_spv(
    device_info: &DeviceInfo,
    bytes: &[u8],
) -> Option<vk::ShaderModule> {
    let create_shader_module = device_info.dispatch.create_shader_module?;
    let code = ash::util::read_spv(&mut Cursor::new(bytes)).ok()?;
    let create_info = vk::ShaderModuleCreateInfo {
        s_type: vk::StructureType::SHADER_MODULE_CREATE_INFO,
        code_size: code.len() * std::mem::size_of::<u32>(),
        p_code: code.as_ptr(),
        ..Default::default()
    };
    let mut shader_module = vk::ShaderModule::null();
    if create_shader_module(
        device_info.device,
        &create_info,
        ptr::null(),
        &mut shader_module,
    ) != vk::Result::SUCCESS
    {
        return None;
    }
    Some(shader_module)
}

unsafe fn create_simple_image_view(
    device_info: &DeviceInfo,
    image: vk::Image,
    format: vk::Format,
) -> Option<vk::ImageView> {
    let create_image_view = device_info.dispatch.create_image_view?;
    let create_info = vk::ImageViewCreateInfo {
        s_type: vk::StructureType::IMAGE_VIEW_CREATE_INFO,
        image,
        view_type: vk::ImageViewType::TYPE_2D,
        format,
        subresource_range: vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        },
        ..Default::default()
    };
    let mut image_view = vk::ImageView::null();
    if create_image_view(
        device_info.device,
        &create_info,
        ptr::null(),
        &mut image_view,
    ) != vk::Result::SUCCESS
    {
        return None;
    }
    Some(image_view)
}

unsafe fn destroy_blend_resources(
    dispatch: &DeviceDispatch,
    device: vk::Device,
    blend: &mut BlendResources,
) {
    if !blend.initialized {
        return;
    }

    if let Some(destroy_image_view) = dispatch.destroy_image_view {
        if !blend.history_view.is_null() {
            destroy_image_view(device, blend.history_view, ptr::null());
            blend.history_view = vk::ImageView::null();
        }
    }
    if let Some(destroy_sampler) = dispatch.destroy_sampler {
        if !blend.sampler.is_null() {
            destroy_sampler(device, blend.sampler, ptr::null());
            blend.sampler = vk::Sampler::null();
        }
    }
    if let Some(destroy_pipeline) = dispatch.destroy_pipeline {
        if !blend.pipeline.is_null() {
            destroy_pipeline(device, blend.pipeline, ptr::null());
            blend.pipeline = vk::Pipeline::null();
        }
    }
    if let Some(destroy_render_pass) = dispatch.destroy_render_pass {
        if !blend.render_pass.is_null() {
            destroy_render_pass(device, blend.render_pass, ptr::null());
            blend.render_pass = vk::RenderPass::null();
        }
    }
    if let Some(destroy_pipeline_layout) = dispatch.destroy_pipeline_layout {
        if !blend.pipeline_layout.is_null() {
            destroy_pipeline_layout(device, blend.pipeline_layout, ptr::null());
            blend.pipeline_layout = vk::PipelineLayout::null();
        }
    }
    if let Some(destroy_descriptor_pool) = dispatch.destroy_descriptor_pool {
        if !blend.descriptor_pool.is_null() {
            destroy_descriptor_pool(device, blend.descriptor_pool, ptr::null());
            blend.descriptor_pool = vk::DescriptorPool::null();
        }
    }
    blend.descriptor_set = vk::DescriptorSet::null();
    if let Some(destroy_descriptor_set_layout) = dispatch.destroy_descriptor_set_layout {
        if !blend.descriptor_set_layout.is_null() {
            destroy_descriptor_set_layout(device, blend.descriptor_set_layout, ptr::null());
            blend.descriptor_set_layout = vk::DescriptorSetLayout::null();
        }
    }
    blend.initialized = false;
}

unsafe fn init_blend_resources(swapchain: &mut SwapchainState, device_info: &DeviceInfo) -> bool {
    if swapchain.blend.initialized {
        return true;
    }
    if !ensure_history_image(swapchain, device_info) {
        return false;
    }

    let (
        Some(create_sampler),
        Some(create_descriptor_set_layout),
        Some(create_descriptor_pool),
        Some(allocate_descriptor_sets),
        Some(update_descriptor_sets),
        Some(create_pipeline_layout),
        Some(create_render_pass),
        Some(create_graphics_pipelines),
        Some(destroy_shader_module),
    ) = (
        device_info.dispatch.create_sampler,
        device_info.dispatch.create_descriptor_set_layout,
        device_info.dispatch.create_descriptor_pool,
        device_info.dispatch.allocate_descriptor_sets,
        device_info.dispatch.update_descriptor_sets,
        device_info.dispatch.create_pipeline_layout,
        device_info.dispatch.create_render_pass,
        device_info.dispatch.create_graphics_pipelines,
        device_info.dispatch.destroy_shader_module,
    )
    else {
        log_warn("blend resource creation functions unavailable");
        return false;
    };

    let mut blend = BlendResources::default();
    blend.history_view =
        match create_simple_image_view(device_info, swapchain.history_image, swapchain.format) {
            Some(view) => view,
            None => {
                log_warn("CreateImageView failed for blend history view");
                return false;
            }
        };

    let sampler_info = vk::SamplerCreateInfo {
        s_type: vk::StructureType::SAMPLER_CREATE_INFO,
        mag_filter: vk::Filter::LINEAR,
        min_filter: vk::Filter::LINEAR,
        mipmap_mode: vk::SamplerMipmapMode::LINEAR,
        address_mode_u: vk::SamplerAddressMode::CLAMP_TO_EDGE,
        address_mode_v: vk::SamplerAddressMode::CLAMP_TO_EDGE,
        address_mode_w: vk::SamplerAddressMode::CLAMP_TO_EDGE,
        anisotropy_enable: vk::FALSE,
        max_anisotropy: 1.0,
        min_lod: 0.0,
        max_lod: 0.0,
        border_color: vk::BorderColor::FLOAT_OPAQUE_BLACK,
        unnormalized_coordinates: vk::FALSE,
        ..Default::default()
    };
    if create_sampler(
        device_info.device,
        &sampler_info,
        ptr::null(),
        &mut blend.sampler,
    ) != vk::Result::SUCCESS
    {
        log_warn("CreateSampler failed for blend resources");
        destroy_blend_resources(&device_info.dispatch, device_info.device, &mut blend);
        return false;
    }

    let bindings = [
        vk::DescriptorSetLayoutBinding {
            binding: 0,
            descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            descriptor_count: 1,
            stage_flags: vk::ShaderStageFlags::FRAGMENT,
            ..Default::default()
        },
        vk::DescriptorSetLayoutBinding {
            binding: 1,
            descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            descriptor_count: 1,
            stage_flags: vk::ShaderStageFlags::FRAGMENT,
            ..Default::default()
        },
    ];
    let descriptor_set_layout_info = vk::DescriptorSetLayoutCreateInfo {
        s_type: vk::StructureType::DESCRIPTOR_SET_LAYOUT_CREATE_INFO,
        binding_count: bindings.len() as u32,
        p_bindings: bindings.as_ptr(),
        ..Default::default()
    };
    if create_descriptor_set_layout(
        device_info.device,
        &descriptor_set_layout_info,
        ptr::null(),
        &mut blend.descriptor_set_layout,
    ) != vk::Result::SUCCESS
    {
        log_warn("CreateDescriptorSetLayout failed for blend resources");
        destroy_blend_resources(&device_info.dispatch, device_info.device, &mut blend);
        return false;
    }

    let pool_size = [vk::DescriptorPoolSize {
        ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
        descriptor_count: 2,
    }];
    let descriptor_pool_info = vk::DescriptorPoolCreateInfo {
        s_type: vk::StructureType::DESCRIPTOR_POOL_CREATE_INFO,
        max_sets: 1,
        pool_size_count: pool_size.len() as u32,
        p_pool_sizes: pool_size.as_ptr(),
        ..Default::default()
    };
    if create_descriptor_pool(
        device_info.device,
        &descriptor_pool_info,
        ptr::null(),
        &mut blend.descriptor_pool,
    ) != vk::Result::SUCCESS
    {
        log_warn("CreateDescriptorPool failed for blend resources");
        destroy_blend_resources(&device_info.dispatch, device_info.device, &mut blend);
        return false;
    }

    let descriptor_set_layouts = [blend.descriptor_set_layout];
    let descriptor_set_alloc_info = vk::DescriptorSetAllocateInfo {
        s_type: vk::StructureType::DESCRIPTOR_SET_ALLOCATE_INFO,
        descriptor_pool: blend.descriptor_pool,
        descriptor_set_count: 1,
        p_set_layouts: descriptor_set_layouts.as_ptr(),
        ..Default::default()
    };
    if allocate_descriptor_sets(
        device_info.device,
        &descriptor_set_alloc_info,
        &mut blend.descriptor_set,
    ) != vk::Result::SUCCESS
    {
        log_warn("AllocateDescriptorSets failed for blend resources");
        destroy_blend_resources(&device_info.dispatch, device_info.device, &mut blend);
        return false;
    }

    let set_layouts = [blend.descriptor_set_layout];
    let pipeline_layout_info = vk::PipelineLayoutCreateInfo {
        s_type: vk::StructureType::PIPELINE_LAYOUT_CREATE_INFO,
        set_layout_count: set_layouts.len() as u32,
        p_set_layouts: set_layouts.as_ptr(),
        ..Default::default()
    };
    if create_pipeline_layout(
        device_info.device,
        &pipeline_layout_info,
        ptr::null(),
        &mut blend.pipeline_layout,
    ) != vk::Result::SUCCESS
    {
        log_warn("CreatePipelineLayout failed for blend resources");
        destroy_blend_resources(&device_info.dispatch, device_info.device, &mut blend);
        return false;
    }

    let attachment_descriptions = [vk::AttachmentDescription {
        format: swapchain.format,
        samples: vk::SampleCountFlags::TYPE_1,
        load_op: vk::AttachmentLoadOp::DONT_CARE,
        store_op: vk::AttachmentStoreOp::STORE,
        stencil_load_op: vk::AttachmentLoadOp::DONT_CARE,
        stencil_store_op: vk::AttachmentStoreOp::DONT_CARE,
        initial_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        final_layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        ..Default::default()
    }];
    let color_attachment_refs = [vk::AttachmentReference {
        attachment: 0,
        layout: vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
    }];
    let subpasses = [vk::SubpassDescription {
        pipeline_bind_point: vk::PipelineBindPoint::GRAPHICS,
        color_attachment_count: 1,
        p_color_attachments: color_attachment_refs.as_ptr(),
        ..Default::default()
    }];
    let subpass_dependencies = [
        vk::SubpassDependency {
            src_subpass: vk::SUBPASS_EXTERNAL,
            dst_subpass: 0,
            src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            dst_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            dst_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            ..Default::default()
        },
        vk::SubpassDependency {
            src_subpass: 0,
            dst_subpass: vk::SUBPASS_EXTERNAL,
            src_stage_mask: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            src_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
            dst_stage_mask: vk::PipelineStageFlags::BOTTOM_OF_PIPE,
            dst_access_mask: vk::AccessFlags::MEMORY_READ,
            ..Default::default()
        },
    ];
    let render_pass_info = vk::RenderPassCreateInfo {
        s_type: vk::StructureType::RENDER_PASS_CREATE_INFO,
        attachment_count: attachment_descriptions.len() as u32,
        p_attachments: attachment_descriptions.as_ptr(),
        subpass_count: subpasses.len() as u32,
        p_subpasses: subpasses.as_ptr(),
        dependency_count: subpass_dependencies.len() as u32,
        p_dependencies: subpass_dependencies.as_ptr(),
        ..Default::default()
    };
    if create_render_pass(
        device_info.device,
        &render_pass_info,
        ptr::null(),
        &mut blend.render_pass,
    ) != vk::Result::SUCCESS
    {
        log_warn("CreateRenderPass failed for blend resources");
        destroy_blend_resources(&device_info.dispatch, device_info.device, &mut blend);
        return false;
    }

    let Some(vert_shader_module) = create_shader_module_from_spv(device_info, BLEND_VERT_SPV)
    else {
        log_warn("failed to create blend vertex shader module");
        destroy_blend_resources(&device_info.dispatch, device_info.device, &mut blend);
        return false;
    };
    let Some(frag_shader_module) = create_shader_module_from_spv(device_info, BLEND_FRAG_SPV)
    else {
        destroy_shader_module(device_info.device, vert_shader_module, ptr::null());
        log_warn("failed to create blend fragment shader module");
        destroy_blend_resources(&device_info.dispatch, device_info.device, &mut blend);
        return false;
    };

    let shader_entry = cstr!("main");
    let shader_stages = [
        vk::PipelineShaderStageCreateInfo {
            s_type: vk::StructureType::PIPELINE_SHADER_STAGE_CREATE_INFO,
            stage: vk::ShaderStageFlags::VERTEX,
            module: vert_shader_module,
            p_name: shader_entry,
            ..Default::default()
        },
        vk::PipelineShaderStageCreateInfo {
            s_type: vk::StructureType::PIPELINE_SHADER_STAGE_CREATE_INFO,
            stage: vk::ShaderStageFlags::FRAGMENT,
            module: frag_shader_module,
            p_name: shader_entry,
            ..Default::default()
        },
    ];
    let vertex_input_state = vk::PipelineVertexInputStateCreateInfo {
        s_type: vk::StructureType::PIPELINE_VERTEX_INPUT_STATE_CREATE_INFO,
        ..Default::default()
    };
    let input_assembly_state = vk::PipelineInputAssemblyStateCreateInfo {
        s_type: vk::StructureType::PIPELINE_INPUT_ASSEMBLY_STATE_CREATE_INFO,
        topology: vk::PrimitiveTopology::TRIANGLE_LIST,
        primitive_restart_enable: vk::FALSE,
        ..Default::default()
    };
    let viewports = [vk::Viewport {
        x: 0.0,
        y: 0.0,
        width: swapchain.extent.width as f32,
        height: swapchain.extent.height as f32,
        min_depth: 0.0,
        max_depth: 1.0,
    }];
    let scissors = [vk::Rect2D {
        offset: vk::Offset2D { x: 0, y: 0 },
        extent: swapchain.extent,
    }];
    let viewport_state = vk::PipelineViewportStateCreateInfo {
        s_type: vk::StructureType::PIPELINE_VIEWPORT_STATE_CREATE_INFO,
        viewport_count: viewports.len() as u32,
        p_viewports: viewports.as_ptr(),
        scissor_count: scissors.len() as u32,
        p_scissors: scissors.as_ptr(),
        ..Default::default()
    };
    let rasterization_state = vk::PipelineRasterizationStateCreateInfo {
        s_type: vk::StructureType::PIPELINE_RASTERIZATION_STATE_CREATE_INFO,
        depth_clamp_enable: vk::FALSE,
        rasterizer_discard_enable: vk::FALSE,
        polygon_mode: vk::PolygonMode::FILL,
        cull_mode: vk::CullModeFlags::NONE,
        front_face: vk::FrontFace::COUNTER_CLOCKWISE,
        line_width: 1.0,
        ..Default::default()
    };
    let multisample_state = vk::PipelineMultisampleStateCreateInfo {
        s_type: vk::StructureType::PIPELINE_MULTISAMPLE_STATE_CREATE_INFO,
        rasterization_samples: vk::SampleCountFlags::TYPE_1,
        ..Default::default()
    };
    let color_blend_attachments = [vk::PipelineColorBlendAttachmentState {
        blend_enable: vk::FALSE,
        src_color_blend_factor: vk::BlendFactor::ONE,
        dst_color_blend_factor: vk::BlendFactor::ZERO,
        color_blend_op: vk::BlendOp::ADD,
        src_alpha_blend_factor: vk::BlendFactor::ONE,
        dst_alpha_blend_factor: vk::BlendFactor::ZERO,
        alpha_blend_op: vk::BlendOp::ADD,
        color_write_mask: vk::ColorComponentFlags::R
            | vk::ColorComponentFlags::G
            | vk::ColorComponentFlags::B
            | vk::ColorComponentFlags::A,
    }];
    let color_blend_state = vk::PipelineColorBlendStateCreateInfo {
        s_type: vk::StructureType::PIPELINE_COLOR_BLEND_STATE_CREATE_INFO,
        attachment_count: color_blend_attachments.len() as u32,
        p_attachments: color_blend_attachments.as_ptr(),
        ..Default::default()
    };
    let pipeline_info = vk::GraphicsPipelineCreateInfo {
        s_type: vk::StructureType::GRAPHICS_PIPELINE_CREATE_INFO,
        stage_count: shader_stages.len() as u32,
        p_stages: shader_stages.as_ptr(),
        p_vertex_input_state: &vertex_input_state,
        p_input_assembly_state: &input_assembly_state,
        p_viewport_state: &viewport_state,
        p_rasterization_state: &rasterization_state,
        p_multisample_state: &multisample_state,
        p_color_blend_state: &color_blend_state,
        layout: blend.pipeline_layout,
        render_pass: blend.render_pass,
        subpass: 0,
        ..Default::default()
    };
    let pipeline_result = create_graphics_pipelines(
        device_info.device,
        vk::PipelineCache::null(),
        1,
        &pipeline_info,
        ptr::null(),
        &mut blend.pipeline,
    );
    destroy_shader_module(device_info.device, vert_shader_module, ptr::null());
    destroy_shader_module(device_info.device, frag_shader_module, ptr::null());
    if pipeline_result != vk::Result::SUCCESS {
        log_warn("CreateGraphicsPipelines failed for blend resources");
        destroy_blend_resources(&device_info.dispatch, device_info.device, &mut blend);
        return false;
    }

    let history_descriptor = [vk::DescriptorImageInfo {
        sampler: blend.sampler,
        image_view: blend.history_view,
        image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
    }];
    let current_descriptor = [vk::DescriptorImageInfo {
        sampler: blend.sampler,
        image_view: blend.history_view,
        image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
    }];
    let writes = [
        vk::WriteDescriptorSet {
            s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
            dst_set: blend.descriptor_set,
            dst_binding: 0,
            descriptor_count: 1,
            descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            p_image_info: history_descriptor.as_ptr(),
            ..Default::default()
        },
        vk::WriteDescriptorSet {
            s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
            dst_set: blend.descriptor_set,
            dst_binding: 1,
            descriptor_count: 1,
            descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            p_image_info: current_descriptor.as_ptr(),
            ..Default::default()
        },
    ];
    update_descriptor_sets(
        device_info.device,
        writes.len() as u32,
        writes.as_ptr(),
        0,
        ptr::null(),
    );

    blend.initialized = true;
    swapchain.blend = blend;
    log_info("initialized blend resources for swapchain");
    true
}

unsafe fn destroy_swapchain_resources(device_info: &DeviceInfo, swapchain: &mut SwapchainState) {
    if let Some(device_wait_idle) = device_info.dispatch.device_wait_idle {
        let _ = device_wait_idle(device_info.device);
    }
    destroy_inject_resources(
        &device_info.dispatch,
        device_info.device,
        &mut swapchain.inject,
    );
    destroy_blend_resources(
        &device_info.dispatch,
        device_info.device,
        &mut swapchain.blend,
    );
    swapchain.history_valid = false;
    if let Some(destroy_image) = device_info.dispatch.destroy_image {
        if !swapchain.history_image.is_null() {
            destroy_image(device_info.device, swapchain.history_image, ptr::null());
            swapchain.history_image = vk::Image::null();
        }
    }
    if let Some(free_memory) = device_info.dispatch.free_memory {
        if !swapchain.history_memory.is_null() {
            free_memory(device_info.device, swapchain.history_memory, ptr::null());
            swapchain.history_memory = vk::DeviceMemory::null();
        }
    }
}

unsafe fn init_inject_resources(
    swapchain: &mut SwapchainState,
    device_info: &DeviceInfo,
    queue_info: &QueueInfo,
) -> bool {
    if swapchain.inject.initialized {
        if swapchain.inject.family_index == queue_info.family_index {
            return true;
        }
        destroy_inject_resources(
            &device_info.dispatch,
            device_info.device,
            &mut swapchain.inject,
        );
    }

    if !queue_info.supports_graphics && !queue_info.supports_transfer {
        log_warn(
            "present queue family has neither graphics nor transfer support; skipping injection",
        );
        return false;
    }

    let (
        Some(create_command_pool),
        Some(allocate_command_buffers),
        Some(create_semaphore),
        Some(create_fence),
    ) = (
        device_info.dispatch.create_command_pool,
        device_info.dispatch.allocate_command_buffers,
        device_info.dispatch.create_semaphore,
        device_info.dispatch.create_fence,
    )
    else {
        log_warn("injection resource creation functions unavailable");
        return false;
    };

    let pool_info = vk::CommandPoolCreateInfo {
        s_type: vk::StructureType::COMMAND_POOL_CREATE_INFO,
        flags: vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
        queue_family_index: queue_info.family_index,
        ..Default::default()
    };
    if create_command_pool(
        device_info.device,
        &pool_info,
        ptr::null(),
        &mut swapchain.inject.command_pool,
    ) != vk::Result::SUCCESS
    {
        log_warn("CreateCommandPool failed for injection resources");
        return false;
    }

    let alloc_info = vk::CommandBufferAllocateInfo {
        s_type: vk::StructureType::COMMAND_BUFFER_ALLOCATE_INFO,
        command_pool: swapchain.inject.command_pool,
        level: vk::CommandBufferLevel::PRIMARY,
        command_buffer_count: 1,
        ..Default::default()
    };
    if allocate_command_buffers(
        device_info.device,
        &alloc_info,
        &mut swapchain.inject.command_buffer,
    ) != vk::Result::SUCCESS
    {
        log_warn("AllocateCommandBuffers failed for injection resources");
        destroy_inject_resources(
            &device_info.dispatch,
            device_info.device,
            &mut swapchain.inject,
        );
        return false;
    }

    let semaphore_info = vk::SemaphoreCreateInfo {
        s_type: vk::StructureType::SEMAPHORE_CREATE_INFO,
        ..Default::default()
    };
    if create_semaphore(
        device_info.device,
        &semaphore_info,
        ptr::null(),
        &mut swapchain.inject.acquire_semaphore,
    ) != vk::Result::SUCCESS
    {
        log_warn("CreateSemaphore failed for acquire semaphore");
        destroy_inject_resources(
            &device_info.dispatch,
            device_info.device,
            &mut swapchain.inject,
        );
        return false;
    }
    if create_semaphore(
        device_info.device,
        &semaphore_info,
        ptr::null(),
        &mut swapchain.inject.ready_original_semaphore,
    ) != vk::Result::SUCCESS
    {
        log_warn("CreateSemaphore failed for original-ready semaphore");
        destroy_inject_resources(
            &device_info.dispatch,
            device_info.device,
            &mut swapchain.inject,
        );
        return false;
    }
    if create_semaphore(
        device_info.device,
        &semaphore_info,
        ptr::null(),
        &mut swapchain.inject.ready_generated_semaphore,
    ) != vk::Result::SUCCESS
    {
        log_warn("CreateSemaphore failed for generated-ready semaphore");
        destroy_inject_resources(
            &device_info.dispatch,
            device_info.device,
            &mut swapchain.inject,
        );
        return false;
    }

    let fence_info = vk::FenceCreateInfo {
        s_type: vk::StructureType::FENCE_CREATE_INFO,
        flags: vk::FenceCreateFlags::SIGNALED,
        ..Default::default()
    };
    if create_fence(
        device_info.device,
        &fence_info,
        ptr::null(),
        &mut swapchain.inject.submit_fence,
    ) != vk::Result::SUCCESS
    {
        log_warn("CreateFence failed for submit fence");
        destroy_inject_resources(
            &device_info.dispatch,
            device_info.device,
            &mut swapchain.inject,
        );
        return false;
    }

    swapchain.inject.initialized = true;
    swapchain.inject.family_index = queue_info.family_index;
    log_info(format!(
        "initialized injection resources for queue family {}",
        queue_info.family_index
    ));
    true
}

unsafe fn refresh_swapchain_images(state: &mut SwapchainState, dispatch: &DeviceDispatch) {
    let Some(get_swapchain_images) = dispatch.get_swapchain_images_khr else {
        return;
    };

    let mut image_count = 0;
    let result = get_swapchain_images(
        state.device,
        state.handle,
        &mut image_count,
        ptr::null_mut(),
    );
    if result != vk::Result::SUCCESS || image_count == 0 {
        log_warn(format!(
            "GetSwapchainImagesKHR(count) failed: {}",
            result.as_raw()
        ));
        return;
    }

    state.images.resize(image_count as usize, vk::Image::null());
    let result = get_swapchain_images(
        state.device,
        state.handle,
        &mut image_count,
        state.images.as_mut_ptr(),
    );
    if result != vk::Result::SUCCESS {
        log_warn(format!(
            "GetSwapchainImagesKHR(images) failed: {}",
            result.as_raw()
        ));
        state.images.clear();
        return;
    }
    state.images.truncate(image_count as usize);
}

fn image_barrier(
    image: vk::Image,
    src_access: vk::AccessFlags,
    dst_access: vk::AccessFlags,
    old_layout: vk::ImageLayout,
    new_layout: vk::ImageLayout,
) -> vk::ImageMemoryBarrier<'static> {
    vk::ImageMemoryBarrier {
        s_type: vk::StructureType::IMAGE_MEMORY_BARRIER,
        src_access_mask: src_access,
        dst_access_mask: dst_access,
        old_layout,
        new_layout,
        src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        image,
        subresource_range: vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        },
        ..Default::default()
    }
}

unsafe fn destroy_ephemeral_blend_frame_resources(
    dispatch: &DeviceDispatch,
    device: vk::Device,
    current_view: &mut vk::ImageView,
    generated_view: &mut vk::ImageView,
    framebuffer: &mut vk::Framebuffer,
) {
    if let Some(destroy_framebuffer) = dispatch.destroy_framebuffer {
        if !framebuffer.is_null() {
            destroy_framebuffer(device, *framebuffer, ptr::null());
            *framebuffer = vk::Framebuffer::null();
        }
    }
    if let Some(destroy_image_view) = dispatch.destroy_image_view {
        if !generated_view.is_null() {
            destroy_image_view(device, *generated_view, ptr::null());
            *generated_view = vk::ImageView::null();
        }
        if !current_view.is_null() {
            destroy_image_view(device, *current_view, ptr::null());
            *current_view = vk::ImageView::null();
        }
    }
}

unsafe fn update_blend_descriptor_set(
    device_info: &DeviceInfo,
    blend: &BlendResources,
    history_view: vk::ImageView,
    current_view: vk::ImageView,
) -> bool {
    let Some(update_descriptor_sets) = device_info.dispatch.update_descriptor_sets else {
        return false;
    };

    let history_descriptor = [vk::DescriptorImageInfo {
        sampler: blend.sampler,
        image_view: history_view,
        image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
    }];
    let current_descriptor = [vk::DescriptorImageInfo {
        sampler: blend.sampler,
        image_view: current_view,
        image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
    }];
    let writes = [
        vk::WriteDescriptorSet {
            s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
            dst_set: blend.descriptor_set,
            dst_binding: 0,
            descriptor_count: 1,
            descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            p_image_info: history_descriptor.as_ptr(),
            ..Default::default()
        },
        vk::WriteDescriptorSet {
            s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
            dst_set: blend.descriptor_set,
            dst_binding: 1,
            descriptor_count: 1,
            descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            p_image_info: current_descriptor.as_ptr(),
            ..Default::default()
        },
    ];
    update_descriptor_sets(
        device_info.device,
        writes.len() as u32,
        writes.as_ptr(),
        0,
        ptr::null(),
    );
    true
}

unsafe fn try_present_blend_frame(
    state: &mut SwapchainState,
    device_info: &DeviceInfo,
    queue_info: &QueueInfo,
    queue: vk::Queue,
    present_info: *const PresentInfoKHR,
) -> bool {
    if !init_inject_resources(state, device_info, queue_info) {
        return false;
    }
    if !init_blend_resources(state, device_info) {
        return false;
    }

    let present_info = match present_info.as_ref() {
        Some(present_info) if present_info.swapchain_count == 1 => present_info,
        _ => return false,
    };

    let (
        Some(wait_for_fences),
        Some(acquire_next_image),
        Some(reset_command_pool),
        Some(begin_command_buffer),
        Some(cmd_pipeline_barrier),
        Some(cmd_copy_image),
        Some(end_command_buffer),
        Some(reset_fences),
        Some(queue_submit),
        Some(queue_present),
        Some(queue_wait_idle),
        Some(create_framebuffer),
        Some(cmd_begin_render_pass),
        Some(cmd_end_render_pass),
        Some(cmd_bind_pipeline),
        Some(cmd_bind_descriptor_sets),
        Some(cmd_draw),
    ) = (
        device_info.dispatch.wait_for_fences,
        device_info.dispatch.acquire_next_image_khr,
        device_info.dispatch.reset_command_pool,
        device_info.dispatch.begin_command_buffer,
        device_info.dispatch.cmd_pipeline_barrier,
        device_info.dispatch.cmd_copy_image,
        device_info.dispatch.end_command_buffer,
        device_info.dispatch.reset_fences,
        device_info.dispatch.queue_submit,
        device_info.dispatch.queue_present_khr,
        device_info.dispatch.queue_wait_idle,
        device_info.dispatch.create_framebuffer,
        device_info.dispatch.cmd_begin_render_pass,
        device_info.dispatch.cmd_end_render_pass,
        device_info.dispatch.cmd_bind_pipeline,
        device_info.dispatch.cmd_bind_descriptor_sets,
        device_info.dispatch.cmd_draw,
    )
    else {
        return false;
    };

    let prior_submit_wait = wait_for_fences(
        device_info.device,
        1,
        &state.inject.submit_fence,
        vk::TRUE,
        5_000_000_000,
    );
    if prior_submit_wait != vk::Result::SUCCESS {
        log_warn(format!(
            "WaitForFences failed for submit fence: {}",
            prior_submit_wait.as_raw()
        ));
        return false;
    }

    let source_index = *present_info.p_image_indices;
    if source_index as usize >= state.images.len() {
        refresh_swapchain_images(state, &device_info.dispatch);
        if source_index as usize >= state.images.len() {
            log_warn("blend source image index out of bounds after refresh");
            return false;
        }
    }
    let source_image = state.images[source_index as usize];

    let have_generated = state.history_valid;
    let mut generated_image_index = 0;
    if have_generated {
        let acquire_result = acquire_next_image(
            device_info.device,
            state.handle,
            20_000_000,
            state.inject.acquire_semaphore,
            vk::Fence::null(),
            &mut generated_image_index,
        );
        if acquire_result == vk::Result::TIMEOUT || acquire_result == vk::Result::NOT_READY {
            log_warn(
                "AcquireNextImageKHR timed out for blend frame; skipping injection this present",
            );
            return false;
        }
        if acquire_result != vk::Result::SUCCESS && acquire_result != vk::Result::SUBOPTIMAL_KHR {
            log_warn(format!(
                "AcquireNextImageKHR failed for blend frame: {}",
                acquire_result.as_raw()
            ));
            return false;
        }
        if generated_image_index as usize >= state.images.len() {
            refresh_swapchain_images(state, &device_info.dispatch);
            if generated_image_index as usize >= state.images.len() {
                log_warn("blend generated image index out of bounds after refresh");
                return false;
            }
        }
        if generated_image_index == source_index {
            log_warn("blend acquire returned current source image index; skipping injection");
            return false;
        }
    }

    let mut current_view = match create_simple_image_view(device_info, source_image, state.format) {
        Some(view) => view,
        None => {
            log_warn("CreateImageView failed for blend current source view");
            return false;
        }
    };
    let mut generated_view = vk::ImageView::null();
    let mut framebuffer = vk::Framebuffer::null();

    if have_generated {
        let generated_image = state.images[generated_image_index as usize];
        generated_view = match create_simple_image_view(device_info, generated_image, state.format)
        {
            Some(view) => view,
            None => {
                log_warn("CreateImageView failed for blend generated target view");
                destroy_ephemeral_blend_frame_resources(
                    &device_info.dispatch,
                    device_info.device,
                    &mut current_view,
                    &mut generated_view,
                    &mut framebuffer,
                );
                return false;
            }
        };
        let attachments = [generated_view];
        let framebuffer_info = vk::FramebufferCreateInfo {
            s_type: vk::StructureType::FRAMEBUFFER_CREATE_INFO,
            render_pass: state.blend.render_pass,
            attachment_count: attachments.len() as u32,
            p_attachments: attachments.as_ptr(),
            width: state.extent.width,
            height: state.extent.height,
            layers: 1,
            ..Default::default()
        };
        if create_framebuffer(
            device_info.device,
            &framebuffer_info,
            ptr::null(),
            &mut framebuffer,
        ) != vk::Result::SUCCESS
        {
            log_warn("CreateFramebuffer failed for blend generated target");
            destroy_ephemeral_blend_frame_resources(
                &device_info.dispatch,
                device_info.device,
                &mut current_view,
                &mut generated_view,
                &mut framebuffer,
            );
            return false;
        }
    }

    if !update_blend_descriptor_set(
        device_info,
        &state.blend,
        state.blend.history_view,
        current_view,
    ) {
        destroy_ephemeral_blend_frame_resources(
            &device_info.dispatch,
            device_info.device,
            &mut current_view,
            &mut generated_view,
            &mut framebuffer,
        );
        return false;
    }

    if reset_command_pool(
        device_info.device,
        state.inject.command_pool,
        vk::CommandPoolResetFlags::empty(),
    ) != vk::Result::SUCCESS
    {
        log_warn("ResetCommandPool failed in blend mode");
        destroy_ephemeral_blend_frame_resources(
            &device_info.dispatch,
            device_info.device,
            &mut current_view,
            &mut generated_view,
            &mut framebuffer,
        );
        return false;
    }
    let begin_info = vk::CommandBufferBeginInfo {
        s_type: vk::StructureType::COMMAND_BUFFER_BEGIN_INFO,
        flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT,
        ..Default::default()
    };
    if begin_command_buffer(state.inject.command_buffer, &begin_info) != vk::Result::SUCCESS {
        log_warn("BeginCommandBuffer failed in blend mode");
        destroy_ephemeral_blend_frame_resources(
            &device_info.dispatch,
            device_info.device,
            &mut current_view,
            &mut generated_view,
            &mut framebuffer,
        );
        return false;
    }

    if have_generated {
        let barriers_before = [
            image_barrier(
                source_image,
                vk::AccessFlags::MEMORY_READ,
                vk::AccessFlags::SHADER_READ,
                vk::ImageLayout::PRESENT_SRC_KHR,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            ),
            image_barrier(
                state.history_image,
                vk::AccessFlags::MEMORY_READ,
                vk::AccessFlags::SHADER_READ,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            ),
            image_barrier(
                state.images[generated_image_index as usize],
                vk::AccessFlags::empty(),
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            ),
        ];
        cmd_pipeline_barrier(
            state.inject.command_buffer,
            vk::PipelineStageFlags::ALL_COMMANDS,
            vk::PipelineStageFlags::FRAGMENT_SHADER
                | vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            vk::DependencyFlags::empty(),
            0,
            ptr::null(),
            0,
            ptr::null(),
            barriers_before.len() as u32,
            barriers_before.as_ptr(),
        );

        let render_area = vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: state.extent,
        };
        let render_pass_info = vk::RenderPassBeginInfo {
            s_type: vk::StructureType::RENDER_PASS_BEGIN_INFO,
            render_pass: state.blend.render_pass,
            framebuffer,
            render_area,
            clear_value_count: 0,
            p_clear_values: ptr::null(),
            ..Default::default()
        };
        cmd_begin_render_pass(
            state.inject.command_buffer,
            &render_pass_info,
            vk::SubpassContents::INLINE,
        );
        cmd_bind_pipeline(
            state.inject.command_buffer,
            vk::PipelineBindPoint::GRAPHICS,
            state.blend.pipeline,
        );
        cmd_bind_descriptor_sets(
            state.inject.command_buffer,
            vk::PipelineBindPoint::GRAPHICS,
            state.blend.pipeline_layout,
            0,
            1,
            &state.blend.descriptor_set,
            0,
            ptr::null(),
        );
        cmd_draw(state.inject.command_buffer, 3, 1, 0, 0);
        cmd_end_render_pass(state.inject.command_buffer);

        let barriers_after_blend = [
            image_barrier(
                source_image,
                vk::AccessFlags::SHADER_READ,
                vk::AccessFlags::TRANSFER_READ,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            ),
            image_barrier(
                state.history_image,
                vk::AccessFlags::SHADER_READ,
                vk::AccessFlags::TRANSFER_WRITE,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            ),
            image_barrier(
                state.images[generated_image_index as usize],
                vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                vk::AccessFlags::MEMORY_READ,
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                vk::ImageLayout::PRESENT_SRC_KHR,
            ),
        ];
        cmd_pipeline_barrier(
            state.inject.command_buffer,
            vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            vk::PipelineStageFlags::TRANSFER,
            vk::DependencyFlags::empty(),
            0,
            ptr::null(),
            0,
            ptr::null(),
            barriers_after_blend.len() as u32,
            barriers_after_blend.as_ptr(),
        );
    } else {
        let barriers_before_prime = [
            image_barrier(
                source_image,
                vk::AccessFlags::MEMORY_READ,
                vk::AccessFlags::TRANSFER_READ,
                vk::ImageLayout::PRESENT_SRC_KHR,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            ),
            image_barrier(
                state.history_image,
                vk::AccessFlags::empty(),
                vk::AccessFlags::TRANSFER_WRITE,
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            ),
        ];
        cmd_pipeline_barrier(
            state.inject.command_buffer,
            vk::PipelineStageFlags::ALL_COMMANDS,
            vk::PipelineStageFlags::TRANSFER,
            vk::DependencyFlags::empty(),
            0,
            ptr::null(),
            0,
            ptr::null(),
            barriers_before_prime.len() as u32,
            barriers_before_prime.as_ptr(),
        );
    }

    let current_to_history = vk::ImageCopy {
        src_subresource: vk::ImageSubresourceLayers {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            mip_level: 0,
            base_array_layer: 0,
            layer_count: 1,
        },
        dst_subresource: vk::ImageSubresourceLayers {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            mip_level: 0,
            base_array_layer: 0,
            layer_count: 1,
        },
        extent: vk::Extent3D {
            width: state.extent.width,
            height: state.extent.height,
            depth: 1,
        },
        ..Default::default()
    };
    cmd_copy_image(
        state.inject.command_buffer,
        source_image,
        vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        state.history_image,
        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        1,
        &current_to_history,
    );

    let barriers_after_copy = [
        image_barrier(
            source_image,
            vk::AccessFlags::TRANSFER_READ,
            vk::AccessFlags::MEMORY_READ,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            vk::ImageLayout::PRESENT_SRC_KHR,
        ),
        image_barrier(
            state.history_image,
            vk::AccessFlags::TRANSFER_WRITE,
            vk::AccessFlags::SHADER_READ,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        ),
    ];
    cmd_pipeline_barrier(
        state.inject.command_buffer,
        vk::PipelineStageFlags::TRANSFER,
        vk::PipelineStageFlags::BOTTOM_OF_PIPE,
        vk::DependencyFlags::empty(),
        0,
        ptr::null(),
        0,
        ptr::null(),
        barriers_after_copy.len() as u32,
        barriers_after_copy.as_ptr(),
    );

    if end_command_buffer(state.inject.command_buffer) != vk::Result::SUCCESS {
        log_warn("EndCommandBuffer failed in blend mode");
        destroy_ephemeral_blend_frame_resources(
            &device_info.dispatch,
            device_info.device,
            &mut current_view,
            &mut generated_view,
            &mut framebuffer,
        );
        return false;
    }

    let mut wait_semaphores = Vec::with_capacity(
        present_info.wait_semaphore_count as usize + if have_generated { 1 } else { 0 },
    );
    let mut wait_stages = Vec::with_capacity(wait_semaphores.capacity());
    for index in 0..present_info.wait_semaphore_count as usize {
        wait_semaphores.push(*present_info.p_wait_semaphores.add(index));
        wait_stages.push(
            vk::PipelineStageFlags::TRANSFER | vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
        );
    }
    if have_generated {
        wait_semaphores.push(state.inject.acquire_semaphore);
        wait_stages.push(vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT);
    }

    let mut signal_semaphores = vec![state.inject.ready_original_semaphore];
    if have_generated {
        signal_semaphores.push(state.inject.ready_generated_semaphore);
    }
    let submit_info = SubmitInfo {
        s_type: vk::StructureType::SUBMIT_INFO,
        wait_semaphore_count: wait_semaphores.len() as u32,
        p_wait_semaphores: if wait_semaphores.is_empty() {
            ptr::null()
        } else {
            wait_semaphores.as_ptr()
        },
        p_wait_dst_stage_mask: if wait_stages.is_empty() {
            ptr::null()
        } else {
            wait_stages.as_ptr()
        },
        command_buffer_count: 1,
        p_command_buffers: &state.inject.command_buffer,
        signal_semaphore_count: signal_semaphores.len() as u32,
        p_signal_semaphores: signal_semaphores.as_ptr(),
        ..Default::default()
    };

    if reset_fences(device_info.device, 1, &state.inject.submit_fence) != vk::Result::SUCCESS {
        log_warn("ResetFences failed for submit fence before blend QueueSubmit");
        destroy_ephemeral_blend_frame_resources(
            &device_info.dispatch,
            device_info.device,
            &mut current_view,
            &mut generated_view,
            &mut framebuffer,
        );
        return false;
    }
    let submit_result = queue_submit(queue, 1, &submit_info, state.inject.submit_fence);
    if submit_result != vk::Result::SUCCESS {
        log_warn(format!(
            "QueueSubmit failed for blend frame: {}",
            submit_result.as_raw()
        ));
        destroy_ephemeral_blend_frame_resources(
            &device_info.dispatch,
            device_info.device,
            &mut current_view,
            &mut generated_view,
            &mut framebuffer,
        );
        return false;
    }

    let first_success = !state.injection_works;
    if have_generated {
        let generated_present = PresentInfoKHR {
            s_type: vk::StructureType::PRESENT_INFO_KHR,
            wait_semaphore_count: 1,
            p_wait_semaphores: &state.inject.ready_generated_semaphore,
            swapchain_count: 1,
            p_swapchains: &state.handle,
            p_image_indices: &generated_image_index,
            ..Default::default()
        };
        let generated_result = queue_present(queue, &generated_present);
        if generated_result != vk::Result::SUCCESS && generated_result != vk::Result::SUBOPTIMAL_KHR
        {
            log_warn(format!(
                "generated QueuePresentKHR failed in blend mode: {}",
                generated_result.as_raw()
            ));
            destroy_ephemeral_blend_frame_resources(
                &device_info.dispatch,
                device_info.device,
                &mut current_view,
                &mut generated_view,
                &mut framebuffer,
            );
            return false;
        }
    }

    let mut original_present = *present_info;
    original_present.wait_semaphore_count = 1;
    original_present.p_wait_semaphores = &state.inject.ready_original_semaphore;
    let original_result = queue_present(queue, &original_present);
    if original_result != vk::Result::SUCCESS && original_result != vk::Result::SUBOPTIMAL_KHR {
        log_warn(format!(
            "original QueuePresentKHR failed in blend mode: {}",
            original_result.as_raw()
        ));
        destroy_ephemeral_blend_frame_resources(
            &device_info.dispatch,
            device_info.device,
            &mut current_view,
            &mut generated_view,
            &mut framebuffer,
        );
        return false;
    }

    if queue_wait_idle(queue) != vk::Result::SUCCESS {
        log_warn("QueueWaitIdle failed in blend mode");
        destroy_ephemeral_blend_frame_resources(
            &device_info.dispatch,
            device_info.device,
            &mut current_view,
            &mut generated_view,
            &mut framebuffer,
        );
        return false;
    }

    destroy_ephemeral_blend_frame_resources(
        &device_info.dispatch,
        device_info.device,
        &mut current_view,
        &mut generated_view,
        &mut framebuffer,
    );

    state.history_valid = true;
    state.injection_works = state.injection_works || have_generated;
    if have_generated {
        state.generated_present_count += 1;
        if first_success {
            log_info("first blended generated-frame present succeeded");
        }
        if state.generated_present_count <= 5 || state.generated_present_count % 60 == 0 {
            log_info(format!(
                "blended frame present={}; generatedImageIndex={}; currentImageIndex={}",
                state.generated_present_count, generated_image_index, source_index
            ));
        }
    } else {
        log_info("blend primed previous frame history");
    }

    true
}

unsafe fn try_present_copy_frame(
    state: &mut SwapchainState,
    device_info: &DeviceInfo,
    queue_info: &QueueInfo,
    queue: vk::Queue,
    present_info: *const PresentInfoKHR,
) -> bool {
    if !init_inject_resources(state, device_info, queue_info) {
        return false;
    }
    let present_info = match present_info.as_ref() {
        Some(present_info)
            if present_info.swapchain_count == 1
                && device_info.dispatch.cmd_copy_image.is_some() =>
        {
            present_info
        }
        _ => return false,
    };

    let Some(wait_for_fences) = device_info.dispatch.wait_for_fences else {
        return false;
    };
    let prior_submit_wait = wait_for_fences(
        device_info.device,
        1,
        &state.inject.submit_fence,
        vk::TRUE,
        5_000_000_000,
    );
    if prior_submit_wait != vk::Result::SUCCESS {
        log_warn(format!(
            "WaitForFences failed for submit fence: {}",
            prior_submit_wait.as_raw()
        ));
        return false;
    }

    let Some(acquire_next_image) = device_info.dispatch.acquire_next_image_khr else {
        return false;
    };
    let mut generated_image_index = 0;
    let acquire_result = acquire_next_image(
        device_info.device,
        state.handle,
        20_000_000,
        state.inject.acquire_semaphore,
        vk::Fence::null(),
        &mut generated_image_index,
    );
    if acquire_result == vk::Result::TIMEOUT || acquire_result == vk::Result::NOT_READY {
        log_warn(
            "AcquireNextImageKHR timed out for duplicate frame; skipping injection this present",
        );
        return false;
    }
    if acquire_result != vk::Result::SUCCESS && acquire_result != vk::Result::SUBOPTIMAL_KHR {
        log_warn(format!(
            "AcquireNextImageKHR failed for duplicate frame: {}",
            acquire_result.as_raw()
        ));
        return false;
    }

    let source_index = unsafe { *present_info.p_image_indices };
    if source_index as usize >= state.images.len()
        || generated_image_index as usize >= state.images.len()
    {
        refresh_swapchain_images(state, &device_info.dispatch);
        if source_index as usize >= state.images.len()
            || generated_image_index as usize >= state.images.len()
        {
            log_warn("copy mode image index out of bounds after refresh");
            return false;
        }
    }
    if generated_image_index == source_index {
        log_warn("duplicate frame acquire returned current source image index; skipping injection");
        return false;
    }

    let source_image = state.images[source_index as usize];
    let generated_image = state.images[generated_image_index as usize];

    let (
        Some(reset_command_pool),
        Some(begin_command_buffer),
        Some(cmd_pipeline_barrier),
        Some(cmd_copy_image),
        Some(end_command_buffer),
        Some(reset_fences),
        Some(queue_submit),
        Some(queue_present),
        Some(queue_wait_idle),
    ) = (
        device_info.dispatch.reset_command_pool,
        device_info.dispatch.begin_command_buffer,
        device_info.dispatch.cmd_pipeline_barrier,
        device_info.dispatch.cmd_copy_image,
        device_info.dispatch.end_command_buffer,
        device_info.dispatch.reset_fences,
        device_info.dispatch.queue_submit,
        device_info.dispatch.queue_present_khr,
        device_info.dispatch.queue_wait_idle,
    )
    else {
        return false;
    };

    if reset_command_pool(
        device_info.device,
        state.inject.command_pool,
        vk::CommandPoolResetFlags::empty(),
    ) != vk::Result::SUCCESS
    {
        log_warn("ResetCommandPool failed in copy mode");
        return false;
    }
    let begin_info = vk::CommandBufferBeginInfo {
        s_type: vk::StructureType::COMMAND_BUFFER_BEGIN_INFO,
        flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT,
        ..Default::default()
    };
    if begin_command_buffer(state.inject.command_buffer, &begin_info) != vk::Result::SUCCESS {
        log_warn("BeginCommandBuffer failed in copy mode");
        return false;
    }

    let barriers_to_copy = [
        image_barrier(
            source_image,
            vk::AccessFlags::MEMORY_READ,
            vk::AccessFlags::TRANSFER_READ,
            vk::ImageLayout::PRESENT_SRC_KHR,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        ),
        image_barrier(
            generated_image,
            vk::AccessFlags::empty(),
            vk::AccessFlags::TRANSFER_WRITE,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        ),
    ];
    cmd_pipeline_barrier(
        state.inject.command_buffer,
        vk::PipelineStageFlags::ALL_COMMANDS,
        vk::PipelineStageFlags::TRANSFER,
        vk::DependencyFlags::empty(),
        0,
        ptr::null(),
        0,
        ptr::null(),
        barriers_to_copy.len() as u32,
        barriers_to_copy.as_ptr(),
    );

    let copy_region = vk::ImageCopy {
        src_subresource: vk::ImageSubresourceLayers {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            mip_level: 0,
            base_array_layer: 0,
            layer_count: 1,
        },
        dst_subresource: vk::ImageSubresourceLayers {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            mip_level: 0,
            base_array_layer: 0,
            layer_count: 1,
        },
        extent: vk::Extent3D {
            width: state.extent.width,
            height: state.extent.height,
            depth: 1,
        },
        ..Default::default()
    };
    cmd_copy_image(
        state.inject.command_buffer,
        source_image,
        vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        generated_image,
        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        1,
        &copy_region,
    );

    let barriers_to_present = [
        image_barrier(
            source_image,
            vk::AccessFlags::TRANSFER_READ,
            vk::AccessFlags::MEMORY_READ,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            vk::ImageLayout::PRESENT_SRC_KHR,
        ),
        image_barrier(
            generated_image,
            vk::AccessFlags::TRANSFER_WRITE,
            vk::AccessFlags::MEMORY_READ,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::ImageLayout::PRESENT_SRC_KHR,
        ),
    ];
    cmd_pipeline_barrier(
        state.inject.command_buffer,
        vk::PipelineStageFlags::TRANSFER,
        vk::PipelineStageFlags::BOTTOM_OF_PIPE,
        vk::DependencyFlags::empty(),
        0,
        ptr::null(),
        0,
        ptr::null(),
        barriers_to_present.len() as u32,
        barriers_to_present.as_ptr(),
    );

    if end_command_buffer(state.inject.command_buffer) != vk::Result::SUCCESS {
        log_warn("EndCommandBuffer failed in copy mode");
        return false;
    }

    let mut wait_semaphores = Vec::with_capacity(present_info.wait_semaphore_count as usize + 1);
    let mut wait_stages = Vec::with_capacity(present_info.wait_semaphore_count as usize + 1);
    for index in 0..present_info.wait_semaphore_count as usize {
        wait_semaphores.push(*present_info.p_wait_semaphores.add(index));
        wait_stages.push(vk::PipelineStageFlags::TRANSFER);
    }
    wait_semaphores.push(state.inject.acquire_semaphore);
    wait_stages.push(vk::PipelineStageFlags::TRANSFER);

    let signal_semaphores = [
        state.inject.ready_original_semaphore,
        state.inject.ready_generated_semaphore,
    ];
    let submit_info = SubmitInfo {
        s_type: vk::StructureType::SUBMIT_INFO,
        wait_semaphore_count: wait_semaphores.len() as u32,
        p_wait_semaphores: wait_semaphores.as_ptr(),
        p_wait_dst_stage_mask: wait_stages.as_ptr(),
        command_buffer_count: 1,
        p_command_buffers: &state.inject.command_buffer,
        signal_semaphore_count: signal_semaphores.len() as u32,
        p_signal_semaphores: signal_semaphores.as_ptr(),
        ..Default::default()
    };

    let first_success = !state.injection_works;
    if reset_fences(device_info.device, 1, &state.inject.submit_fence) != vk::Result::SUCCESS {
        log_warn("ResetFences failed for submit fence before copy QueueSubmit");
        return false;
    }
    let submit_result = queue_submit(queue, 1, &submit_info, state.inject.submit_fence);
    if submit_result != vk::Result::SUCCESS {
        log_warn(format!(
            "QueueSubmit failed for duplicate frame: {}",
            submit_result.as_raw()
        ));
        return false;
    }

    let mut original_present = *present_info;
    original_present.wait_semaphore_count = 1;
    original_present.p_wait_semaphores = &state.inject.ready_original_semaphore;
    let original_result = queue_present(queue, &original_present);
    if original_result != vk::Result::SUCCESS && original_result != vk::Result::SUBOPTIMAL_KHR {
        log_warn(format!(
            "original QueuePresentKHR failed in copy mode: {}",
            original_result.as_raw()
        ));
        return false;
    }

    let generated_present = PresentInfoKHR {
        s_type: vk::StructureType::PRESENT_INFO_KHR,
        wait_semaphore_count: 1,
        p_wait_semaphores: &state.inject.ready_generated_semaphore,
        swapchain_count: 1,
        p_swapchains: &state.handle,
        p_image_indices: &generated_image_index,
        ..Default::default()
    };
    let generated_result = queue_present(queue, &generated_present);
    if generated_result != vk::Result::SUCCESS && generated_result != vk::Result::SUBOPTIMAL_KHR {
        log_warn(format!(
            "generated QueuePresentKHR failed in copy mode: {}",
            generated_result.as_raw()
        ));
        return false;
    }

    if queue_wait_idle(queue) != vk::Result::SUCCESS {
        log_warn("QueueWaitIdle failed in copy mode");
        return false;
    }

    state.injection_works = true;
    state.generated_present_count += 1;
    if first_success {
        log_info("first duplicated-frame present succeeded");
    }
    if state.generated_present_count <= 5 || state.generated_present_count % 60 == 0 {
        log_info(format!(
            "duplicated frame present={}; sourceImageIndex={}; generatedImageIndex={}",
            state.generated_present_count, source_index, generated_image_index
        ));
    }
    true
}

unsafe fn try_present_history_copy_frame(
    state: &mut SwapchainState,
    device_info: &DeviceInfo,
    queue_info: &QueueInfo,
    queue: vk::Queue,
    present_info: *const PresentInfoKHR,
) -> bool {
    if !init_inject_resources(state, device_info, queue_info) {
        return false;
    }
    let present_info = match present_info.as_ref() {
        Some(present_info)
            if present_info.swapchain_count == 1
                && device_info.dispatch.cmd_copy_image.is_some() =>
        {
            present_info
        }
        _ => return false,
    };
    if !ensure_history_image(state, device_info) {
        return false;
    }

    let (
        Some(wait_for_fences),
        Some(acquire_next_image),
        Some(reset_command_pool),
        Some(begin_command_buffer),
        Some(cmd_pipeline_barrier),
        Some(cmd_copy_image),
        Some(end_command_buffer),
        Some(reset_fences),
        Some(queue_submit),
        Some(queue_present),
        Some(queue_wait_idle),
    ) = (
        device_info.dispatch.wait_for_fences,
        device_info.dispatch.acquire_next_image_khr,
        device_info.dispatch.reset_command_pool,
        device_info.dispatch.begin_command_buffer,
        device_info.dispatch.cmd_pipeline_barrier,
        device_info.dispatch.cmd_copy_image,
        device_info.dispatch.end_command_buffer,
        device_info.dispatch.reset_fences,
        device_info.dispatch.queue_submit,
        device_info.dispatch.queue_present_khr,
        device_info.dispatch.queue_wait_idle,
    )
    else {
        return false;
    };

    let prior_submit_wait = wait_for_fences(
        device_info.device,
        1,
        &state.inject.submit_fence,
        vk::TRUE,
        5_000_000_000,
    );
    if prior_submit_wait != vk::Result::SUCCESS {
        log_warn(format!(
            "WaitForFences failed for submit fence: {}",
            prior_submit_wait.as_raw()
        ));
        return false;
    }

    let source_index = *present_info.p_image_indices;
    if source_index as usize >= state.images.len() {
        refresh_swapchain_images(state, &device_info.dispatch);
        if source_index as usize >= state.images.len() {
            log_warn("history-copy source image index out of bounds after refresh");
            return false;
        }
    }
    let source_image = state.images[source_index as usize];

    let have_generated = state.history_valid;
    let mut generated_image_index = 0;
    if have_generated {
        let acquire_result = acquire_next_image(
            device_info.device,
            state.handle,
            20_000_000,
            state.inject.acquire_semaphore,
            vk::Fence::null(),
            &mut generated_image_index,
        );
        if acquire_result == vk::Result::TIMEOUT || acquire_result == vk::Result::NOT_READY {
            log_warn("AcquireNextImageKHR timed out for history-copy frame; skipping injection this present");
            return false;
        }
        if acquire_result != vk::Result::SUCCESS && acquire_result != vk::Result::SUBOPTIMAL_KHR {
            log_warn(format!(
                "AcquireNextImageKHR failed for history-copy frame: {}",
                acquire_result.as_raw()
            ));
            return false;
        }
        if generated_image_index as usize >= state.images.len() {
            refresh_swapchain_images(state, &device_info.dispatch);
            if generated_image_index as usize >= state.images.len() {
                log_warn("history-copy generated image index out of bounds after refresh");
                return false;
            }
        }
    }

    if reset_command_pool(
        device_info.device,
        state.inject.command_pool,
        vk::CommandPoolResetFlags::empty(),
    ) != vk::Result::SUCCESS
    {
        log_warn("ResetCommandPool failed in history-copy mode");
        return false;
    }
    let begin_info = vk::CommandBufferBeginInfo {
        s_type: vk::StructureType::COMMAND_BUFFER_BEGIN_INFO,
        flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT,
        ..Default::default()
    };
    if begin_command_buffer(state.inject.command_buffer, &begin_info) != vk::Result::SUCCESS {
        log_warn("BeginCommandBuffer failed in history-copy mode");
        return false;
    }

    let mut barriers_before = Vec::with_capacity(3);
    barriers_before.push(image_barrier(
        source_image,
        vk::AccessFlags::MEMORY_READ,
        vk::AccessFlags::TRANSFER_READ,
        vk::ImageLayout::PRESENT_SRC_KHR,
        vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
    ));
    if have_generated {
        barriers_before.push(image_barrier(
            state.history_image,
            vk::AccessFlags::MEMORY_READ,
            vk::AccessFlags::TRANSFER_READ,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        ));
        barriers_before.push(image_barrier(
            state.images[generated_image_index as usize],
            vk::AccessFlags::empty(),
            vk::AccessFlags::TRANSFER_WRITE,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        ));
    }
    cmd_pipeline_barrier(
        state.inject.command_buffer,
        vk::PipelineStageFlags::ALL_COMMANDS,
        vk::PipelineStageFlags::TRANSFER,
        vk::DependencyFlags::empty(),
        0,
        ptr::null(),
        0,
        ptr::null(),
        barriers_before.len() as u32,
        barriers_before.as_ptr(),
    );

    if have_generated {
        let previous_copy = vk::ImageCopy {
            src_subresource: vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                mip_level: 0,
                base_array_layer: 0,
                layer_count: 1,
            },
            dst_subresource: vk::ImageSubresourceLayers {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                mip_level: 0,
                base_array_layer: 0,
                layer_count: 1,
            },
            extent: vk::Extent3D {
                width: state.extent.width,
                height: state.extent.height,
                depth: 1,
            },
            ..Default::default()
        };
        cmd_copy_image(
            state.inject.command_buffer,
            state.history_image,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
            state.images[generated_image_index as usize],
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            1,
            &previous_copy,
        );
    }

    let history_to_dst = image_barrier(
        state.history_image,
        if have_generated {
            vk::AccessFlags::TRANSFER_READ
        } else {
            vk::AccessFlags::empty()
        },
        vk::AccessFlags::TRANSFER_WRITE,
        if state.history_valid {
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL
        } else {
            vk::ImageLayout::UNDEFINED
        },
        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
    );
    cmd_pipeline_barrier(
        state.inject.command_buffer,
        vk::PipelineStageFlags::TRANSFER,
        vk::PipelineStageFlags::TRANSFER,
        vk::DependencyFlags::empty(),
        0,
        ptr::null(),
        0,
        ptr::null(),
        1,
        &history_to_dst,
    );

    let current_to_history = vk::ImageCopy {
        src_subresource: vk::ImageSubresourceLayers {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            mip_level: 0,
            base_array_layer: 0,
            layer_count: 1,
        },
        dst_subresource: vk::ImageSubresourceLayers {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            mip_level: 0,
            base_array_layer: 0,
            layer_count: 1,
        },
        extent: vk::Extent3D {
            width: state.extent.width,
            height: state.extent.height,
            depth: 1,
        },
        ..Default::default()
    };
    cmd_copy_image(
        state.inject.command_buffer,
        source_image,
        vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        state.history_image,
        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        1,
        &current_to_history,
    );

    let mut barriers_after = Vec::with_capacity(3);
    barriers_after.push(image_barrier(
        source_image,
        vk::AccessFlags::TRANSFER_READ,
        vk::AccessFlags::MEMORY_READ,
        vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        vk::ImageLayout::PRESENT_SRC_KHR,
    ));
    barriers_after.push(image_barrier(
        state.history_image,
        vk::AccessFlags::TRANSFER_WRITE,
        vk::AccessFlags::MEMORY_READ,
        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
    ));
    if have_generated {
        barriers_after.push(image_barrier(
            state.images[generated_image_index as usize],
            vk::AccessFlags::TRANSFER_WRITE,
            vk::AccessFlags::MEMORY_READ,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::ImageLayout::PRESENT_SRC_KHR,
        ));
    }
    cmd_pipeline_barrier(
        state.inject.command_buffer,
        vk::PipelineStageFlags::TRANSFER,
        vk::PipelineStageFlags::BOTTOM_OF_PIPE,
        vk::DependencyFlags::empty(),
        0,
        ptr::null(),
        0,
        ptr::null(),
        barriers_after.len() as u32,
        barriers_after.as_ptr(),
    );

    if end_command_buffer(state.inject.command_buffer) != vk::Result::SUCCESS {
        log_warn("EndCommandBuffer failed in history-copy mode");
        return false;
    }

    let mut wait_semaphores = Vec::with_capacity(
        present_info.wait_semaphore_count as usize + if have_generated { 1 } else { 0 },
    );
    let mut wait_stages = Vec::with_capacity(wait_semaphores.capacity());
    for index in 0..present_info.wait_semaphore_count as usize {
        wait_semaphores.push(*present_info.p_wait_semaphores.add(index));
        wait_stages.push(vk::PipelineStageFlags::TRANSFER);
    }
    if have_generated {
        wait_semaphores.push(state.inject.acquire_semaphore);
        wait_stages.push(vk::PipelineStageFlags::TRANSFER);
    }

    let mut signal_semaphores = vec![state.inject.ready_original_semaphore];
    if have_generated {
        signal_semaphores.push(state.inject.ready_generated_semaphore);
    }

    let submit_info = SubmitInfo {
        s_type: vk::StructureType::SUBMIT_INFO,
        wait_semaphore_count: wait_semaphores.len() as u32,
        p_wait_semaphores: if wait_semaphores.is_empty() {
            ptr::null()
        } else {
            wait_semaphores.as_ptr()
        },
        p_wait_dst_stage_mask: if wait_stages.is_empty() {
            ptr::null()
        } else {
            wait_stages.as_ptr()
        },
        command_buffer_count: 1,
        p_command_buffers: &state.inject.command_buffer,
        signal_semaphore_count: signal_semaphores.len() as u32,
        p_signal_semaphores: signal_semaphores.as_ptr(),
        ..Default::default()
    };

    if reset_fences(device_info.device, 1, &state.inject.submit_fence) != vk::Result::SUCCESS {
        log_warn("ResetFences failed for submit fence before history-copy QueueSubmit");
        return false;
    }
    let submit_result = queue_submit(queue, 1, &submit_info, state.inject.submit_fence);
    if submit_result != vk::Result::SUCCESS {
        log_warn(format!(
            "QueueSubmit failed for history-copy frame: {}",
            submit_result.as_raw()
        ));
        return false;
    }

    let first_success = !state.injection_works;
    if have_generated {
        let generated_present = PresentInfoKHR {
            s_type: vk::StructureType::PRESENT_INFO_KHR,
            wait_semaphore_count: 1,
            p_wait_semaphores: &state.inject.ready_generated_semaphore,
            swapchain_count: 1,
            p_swapchains: &state.handle,
            p_image_indices: &generated_image_index,
            ..Default::default()
        };
        let generated_result = queue_present(queue, &generated_present);
        if generated_result != vk::Result::SUCCESS && generated_result != vk::Result::SUBOPTIMAL_KHR
        {
            log_warn(format!(
                "generated QueuePresentKHR failed in history-copy mode: {}",
                generated_result.as_raw()
            ));
            return false;
        }
    }

    let mut original_present = *present_info;
    original_present.wait_semaphore_count = 1;
    original_present.p_wait_semaphores = &state.inject.ready_original_semaphore;
    let original_result = queue_present(queue, &original_present);
    if original_result != vk::Result::SUCCESS && original_result != vk::Result::SUBOPTIMAL_KHR {
        log_warn(format!(
            "original QueuePresentKHR failed in history-copy mode: {}",
            original_result.as_raw()
        ));
        return false;
    }

    if queue_wait_idle(queue) != vk::Result::SUCCESS {
        log_warn("QueueWaitIdle failed in history-copy mode");
        return false;
    }

    state.history_valid = true;
    state.injection_works = state.injection_works || have_generated;
    if have_generated {
        state.generated_present_count += 1;
        if first_success {
            log_info("first previous-frame insertion present succeeded");
        }
        if state.generated_present_count <= 5 || state.generated_present_count % 60 == 0 {
            log_info(format!(
                "history-copy generated frame present={}; previousFrameSourceStored=1; generatedImageIndex={}; currentImageIndex={}",
                state.generated_present_count, generated_image_index, source_index
            ));
        }
    } else {
        log_info("history-copy primed previous frame history");
    }

    true
}

unsafe fn try_present_clear_frame(
    state: &mut SwapchainState,
    device_info: &DeviceInfo,
    queue_info: &QueueInfo,
    queue: vk::Queue,
) -> bool {
    if !init_inject_resources(state, device_info, queue_info) {
        return false;
    }

    let (
        Some(wait_for_fences),
        Some(acquire_next_image),
        Some(reset_command_pool),
        Some(begin_command_buffer),
        Some(cmd_pipeline_barrier),
        Some(cmd_clear_color_image),
        Some(end_command_buffer),
        Some(reset_fences),
        Some(queue_submit),
        Some(queue_present),
    ) = (
        device_info.dispatch.wait_for_fences,
        device_info.dispatch.acquire_next_image_khr,
        device_info.dispatch.reset_command_pool,
        device_info.dispatch.begin_command_buffer,
        device_info.dispatch.cmd_pipeline_barrier,
        device_info.dispatch.cmd_clear_color_image,
        device_info.dispatch.end_command_buffer,
        device_info.dispatch.reset_fences,
        device_info.dispatch.queue_submit,
        device_info.dispatch.queue_present_khr,
    )
    else {
        return false;
    };

    let prior_submit_wait = wait_for_fences(
        device_info.device,
        1,
        &state.inject.submit_fence,
        vk::TRUE,
        5_000_000_000,
    );
    if prior_submit_wait != vk::Result::SUCCESS {
        log_warn(format!(
            "WaitForFences failed for submit fence: {}",
            prior_submit_wait.as_raw()
        ));
        return false;
    }

    let mut generated_image_index = 0;
    let acquire_result = acquire_next_image(
        device_info.device,
        state.handle,
        20_000_000,
        state.inject.acquire_semaphore,
        vk::Fence::null(),
        &mut generated_image_index,
    );
    if acquire_result == vk::Result::TIMEOUT || acquire_result == vk::Result::NOT_READY {
        log_warn(
            "AcquireNextImageKHR timed out for generated frame; skipping injection this present",
        );
        return false;
    }
    if acquire_result != vk::Result::SUCCESS && acquire_result != vk::Result::SUBOPTIMAL_KHR {
        log_warn(format!(
            "AcquireNextImageKHR failed for generated frame: {}",
            acquire_result.as_raw()
        ));
        return false;
    }

    if generated_image_index as usize >= state.images.len() {
        refresh_swapchain_images(state, &device_info.dispatch);
        if generated_image_index as usize >= state.images.len() {
            log_warn("generated image index out of bounds after refresh");
            return false;
        }
    }
    let generated_image = state.images[generated_image_index as usize];

    if reset_command_pool(
        device_info.device,
        state.inject.command_pool,
        vk::CommandPoolResetFlags::empty(),
    ) != vk::Result::SUCCESS
    {
        log_warn("ResetCommandPool failed");
        return false;
    }
    let begin_info = vk::CommandBufferBeginInfo {
        s_type: vk::StructureType::COMMAND_BUFFER_BEGIN_INFO,
        flags: vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT,
        ..Default::default()
    };
    if begin_command_buffer(state.inject.command_buffer, &begin_info) != vk::Result::SUCCESS {
        log_warn("BeginCommandBuffer failed");
        return false;
    }

    let to_transfer_dst = image_barrier(
        generated_image,
        vk::AccessFlags::empty(),
        vk::AccessFlags::TRANSFER_WRITE,
        vk::ImageLayout::UNDEFINED,
        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
    );
    cmd_pipeline_barrier(
        state.inject.command_buffer,
        vk::PipelineStageFlags::TOP_OF_PIPE,
        vk::PipelineStageFlags::TRANSFER,
        vk::DependencyFlags::empty(),
        0,
        ptr::null(),
        0,
        ptr::null(),
        1,
        &to_transfer_dst,
    );

    let pulse = if (state.present_count % 120) < 60 {
        0.85_f32
    } else {
        0.15_f32
    };
    let clear_color = vk::ClearColorValue {
        float32: [0.0, pulse, 0.0, 1.0],
    };
    let range = vk::ImageSubresourceRange {
        aspect_mask: vk::ImageAspectFlags::COLOR,
        base_mip_level: 0,
        level_count: 1,
        base_array_layer: 0,
        layer_count: 1,
    };
    cmd_clear_color_image(
        state.inject.command_buffer,
        generated_image,
        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        &clear_color,
        1,
        &range,
    );

    let to_present = image_barrier(
        generated_image,
        vk::AccessFlags::TRANSFER_WRITE,
        vk::AccessFlags::MEMORY_READ,
        vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        vk::ImageLayout::PRESENT_SRC_KHR,
    );
    cmd_pipeline_barrier(
        state.inject.command_buffer,
        vk::PipelineStageFlags::TRANSFER,
        vk::PipelineStageFlags::BOTTOM_OF_PIPE,
        vk::DependencyFlags::empty(),
        0,
        ptr::null(),
        0,
        ptr::null(),
        1,
        &to_present,
    );

    if end_command_buffer(state.inject.command_buffer) != vk::Result::SUCCESS {
        log_warn("EndCommandBuffer failed");
        return false;
    }

    let wait_stage = vk::PipelineStageFlags::TRANSFER;
    let submit_info = SubmitInfo {
        s_type: vk::StructureType::SUBMIT_INFO,
        wait_semaphore_count: 1,
        p_wait_semaphores: &state.inject.acquire_semaphore,
        p_wait_dst_stage_mask: &wait_stage,
        command_buffer_count: 1,
        p_command_buffers: &state.inject.command_buffer,
        ..Default::default()
    };

    let first_success = !state.injection_works;
    if reset_fences(device_info.device, 1, &state.inject.submit_fence) != vk::Result::SUCCESS {
        log_warn("ResetFences failed for submit fence before QueueSubmit");
        return false;
    }
    let submit_result = queue_submit(queue, 1, &submit_info, state.inject.submit_fence);
    if submit_result != vk::Result::SUCCESS {
        log_warn(format!(
            "QueueSubmit failed for generated frame: {}",
            submit_result.as_raw()
        ));
        return false;
    }

    let submit_wait = wait_for_fences(
        device_info.device,
        1,
        &state.inject.submit_fence,
        vk::TRUE,
        5_000_000_000,
    );
    if submit_wait != vk::Result::SUCCESS {
        log_warn(format!(
            "WaitForFences failed after generated frame submit: {}",
            submit_wait.as_raw()
        ));
        return false;
    }

    let generated_present = PresentInfoKHR {
        s_type: vk::StructureType::PRESENT_INFO_KHR,
        swapchain_count: 1,
        p_swapchains: &state.handle,
        p_image_indices: &generated_image_index,
        ..Default::default()
    };
    let generated_present_result = queue_present(queue, &generated_present);
    if generated_present_result != vk::Result::SUCCESS
        && generated_present_result != vk::Result::SUBOPTIMAL_KHR
    {
        log_warn(format!(
            "generated QueuePresentKHR failed: {}",
            generated_present_result.as_raw()
        ));
        return false;
    }

    state.injection_works = true;
    state.generated_present_count += 1;
    if first_success {
        log_info("first generated clear-frame present succeeded");
    }
    if state.generated_present_count <= 5 || state.generated_present_count % 60 == 0 {
        log_info(format!(
            "generated frame present={}; swapchainImageIndex={}",
            state.generated_present_count, generated_image_index
        ));
    }
    true
}

unsafe fn find_instance_layer_link(
    create_info: *const InstanceCreateInfo,
) -> *mut VkLayerInstanceCreateInfo {
    let mut layer_info = (*create_info).p_next as *mut VkLayerInstanceCreateInfo;
    while !layer_info.is_null() {
        if (*layer_info).s_type == vk::StructureType::LOADER_INSTANCE_CREATE_INFO
            && (*layer_info).function == VkLayerFunction::LinkInfo
        {
            return layer_info;
        }
        layer_info = (*layer_info).p_next as *mut VkLayerInstanceCreateInfo;
    }
    ptr::null_mut()
}

unsafe fn find_device_layer_link(
    create_info: *const DeviceCreateInfo,
) -> *mut VkLayerDeviceCreateInfo {
    let mut layer_info = (*create_info).p_next as *mut VkLayerDeviceCreateInfo;
    while !layer_info.is_null() {
        if (*layer_info).s_type == vk::StructureType::LOADER_DEVICE_CREATE_INFO
            && (*layer_info).function == VkLayerFunction::LinkInfo
        {
            return layer_info;
        }
        layer_info = (*layer_info).p_next as *mut VkLayerDeviceCreateInfo;
    }
    ptr::null_mut()
}

unsafe extern "system" fn layer_create_instance(
    create_info: *const InstanceCreateInfo,
    allocator: *const AllocationCallbacks,
    instance: *mut vk::Instance,
) -> vk::Result {
    let mode = Mode::from_env();
    let layer_info = find_instance_layer_link(create_info);
    if layer_info.is_null() {
        log_error("vkCreateInstance: failed to find next layer link info");
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    }
    let link = (*layer_info).u.p_layer_info;
    if link.is_null() {
        log_error("vkCreateInstance: failed to find next layer link info");
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    }

    let next_gipa = (*link).pfn_next_get_instance_proc_addr;
    (*layer_info).u.p_layer_info = (*link).p_next;

    let next_create_instance: Option<PfnVkCreateInstance> =
        load_instance_fn(next_gipa, vk::Instance::null(), cstr!("vkCreateInstance"));
    let Some(next_create_instance) = next_create_instance else {
        log_error("vkCreateInstance: next vkCreateInstance lookup failed");
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    };

    let result = next_create_instance(create_info, allocator, instance);
    if result != vk::Result::SUCCESS {
        log_warn(format!("vkCreateInstance returned {}", result.as_raw()));
        return result;
    }

    let dispatch = fill_instance_dispatch(*instance, next_gipa);
    let key = dispatch_key_from_handle(*instance);
    {
        let mut state = global_state().lock().expect("global state mutex poisoned");
        state.instance_dispatch.insert(key, dispatch);
        state.instance_map.insert(key, *instance);
    }

    let mut message = format!("vkCreateInstance ok; mode={}", mode.name());
    if let Some(app_info) = (*create_info).p_application_info.as_ref() {
        if let Some(app_name) = cstr_opt(app_info.p_application_name) {
            message.push_str(&format!("; app={app_name}"));
        }
        if let Some(engine_name) = cstr_opt(app_info.p_engine_name) {
            message.push_str(&format!("; engine={engine_name}"));
        }
        message.push_str(&format!(
            "; apiVersion={}.{}.{}",
            vk::api_version_major(app_info.api_version),
            vk::api_version_minor(app_info.api_version),
            vk::api_version_patch(app_info.api_version)
        ));
    }
    log_info(message);
    vk::Result::SUCCESS
}

unsafe extern "system" fn layer_destroy_instance(
    instance: vk::Instance,
    allocator: *const AllocationCallbacks,
) {
    if instance.is_null() {
        return;
    }

    let key = dispatch_key_from_handle(instance);
    let dispatch = {
        let mut state = global_state().lock().expect("global state mutex poisoned");
        state.instance_map.remove(&key);
        state.instance_dispatch.remove(&key).unwrap_or_default()
    };

    log_info("vkDestroyInstance");
    if let Some(destroy_instance) = dispatch.destroy_instance {
        destroy_instance(instance, allocator);
    }
}

unsafe extern "system" fn layer_create_device(
    physical_device: vk::PhysicalDevice,
    create_info: *const DeviceCreateInfo,
    allocator: *const AllocationCallbacks,
    device: *mut vk::Device,
) -> vk::Result {
    let layer_info = find_device_layer_link(create_info);
    if layer_info.is_null() {
        log_error("vkCreateDevice: failed to find next layer link info");
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    }
    let link = (*layer_info).u.p_layer_info;
    if link.is_null() {
        log_error("vkCreateDevice: failed to find next layer link info");
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    }

    let next_gipa = (*link).pfn_next_get_instance_proc_addr;
    let next_gdpa = (*link).pfn_next_get_device_proc_addr;
    (*layer_info).u.p_layer_info = (*link).p_next;

    let next_create_device: Option<PfnVkCreateDevice> =
        load_instance_fn(next_gipa, vk::Instance::null(), cstr!("vkCreateDevice"));
    let Some(next_create_device) = next_create_device else {
        log_error("vkCreateDevice: next vkCreateDevice lookup failed");
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    };

    let result = next_create_device(physical_device, create_info, allocator, device);
    if result != vk::Result::SUCCESS {
        log_warn(format!("vkCreateDevice returned {}", result.as_raw()));
        return result;
    }

    let physical_key = dispatch_key_from_handle(physical_device);
    let (instance_dispatch, instance) = {
        let state = global_state().lock().expect("global state mutex poisoned");
        (
            state
                .instance_dispatch
                .get(&physical_key)
                .copied()
                .unwrap_or_default(),
            state
                .instance_map
                .get(&physical_key)
                .copied()
                .unwrap_or(vk::Instance::null()),
        )
    };

    let device_dispatch = fill_device_dispatch(*device, next_gdpa);
    let device_info = DeviceInfo {
        instance,
        physical_device,
        device: *device,
        instance_dispatch,
        dispatch: device_dispatch,
    };

    if let (Some(get_queue_family_properties), Some(get_device_queue)) = (
        instance_dispatch.get_physical_device_queue_family_properties,
        device_dispatch.get_device_queue,
    ) {
        let mut queue_family_count = 0;
        get_queue_family_properties(physical_device, &mut queue_family_count, ptr::null_mut());
        let mut queue_families =
            vec![vk::QueueFamilyProperties::default(); queue_family_count as usize];
        get_queue_family_properties(
            physical_device,
            &mut queue_family_count,
            queue_families.as_mut_ptr(),
        );

        for index in 0..(*create_info).queue_create_info_count as usize {
            let queue_create_info = *(*create_info).p_queue_create_infos.add(index);
            let supports_graphics = queue_families
                .get(queue_create_info.queue_family_index as usize)
                .map(|family| family.queue_flags.contains(vk::QueueFlags::GRAPHICS))
                .unwrap_or(false);
            let supports_transfer = queue_families
                .get(queue_create_info.queue_family_index as usize)
                .map(|family| family.queue_flags.contains(vk::QueueFlags::TRANSFER))
                .unwrap_or(false);

            for queue_index in 0..queue_create_info.queue_count {
                let mut queue = vk::Queue::null();
                get_device_queue(
                    *device,
                    queue_create_info.queue_family_index,
                    queue_index,
                    &mut queue,
                );
                if !queue.is_null() {
                    remember_queue(
                        queue,
                        *device,
                        queue_create_info.queue_family_index,
                        queue_index,
                        supports_graphics,
                        supports_transfer,
                    );
                }
            }
        }
    }

    let mut properties = vk::PhysicalDeviceProperties::default();
    if let Some(get_properties) = instance_dispatch.get_physical_device_properties {
        get_properties(physical_device, &mut properties);
    }

    {
        let mut state = global_state().lock().expect("global state mutex poisoned");
        state
            .device_map
            .insert(dispatch_key_from_handle(*device), device_info);
    }

    let device_name = CStr::from_ptr(properties.device_name.as_ptr())
        .to_string_lossy()
        .into_owned();
    log_info(format!("vkCreateDevice ok; gpu={device_name}"));
    vk::Result::SUCCESS
}

unsafe extern "system" fn layer_destroy_device(
    device: vk::Device,
    allocator: *const AllocationCallbacks,
) {
    if device.is_null() {
        return;
    }

    let key = dispatch_key_from_handle(device);
    let (device_info, found, swapchain_keys) = {
        let mut state = global_state().lock().expect("global state mutex poisoned");
        let device_info = state.device_map.remove(&key).unwrap_or_default();
        let found = !device_info.device.is_null();
        state
            .queue_map
            .retain(|_, queue_info| queue_info.device != device);
        let swapchain_keys: Vec<u64> = state
            .swapchains
            .iter()
            .filter_map(|(swapchain_key, swapchain)| {
                (swapchain.device == device).then_some(*swapchain_key)
            })
            .collect();
        (device_info, found, swapchain_keys)
    };

    if found {
        let mut state = global_state().lock().expect("global state mutex poisoned");
        for swapchain_key in swapchain_keys {
            if let Some(mut swapchain) = state.swapchains.remove(&swapchain_key) {
                destroy_swapchain_resources(&device_info, &mut swapchain);
            }
        }
    }

    log_info("vkDestroyDevice");
    if found {
        if let Some(destroy_device) = device_info.dispatch.destroy_device {
            destroy_device(device, allocator);
        }
    }
}

unsafe extern "system" fn layer_get_device_queue(
    device: vk::Device,
    queue_family_index: u32,
    queue_index: u32,
    queue: *mut vk::Queue,
) {
    let device_info = {
        let state = global_state().lock().expect("global state mutex poisoned");
        state
            .device_map
            .get(&dispatch_key_from_handle(device))
            .copied()
            .unwrap_or_default()
    };
    let Some(get_device_queue) = device_info.dispatch.get_device_queue else {
        return;
    };
    get_device_queue(device, queue_family_index, queue_index, queue);

    let mut supports_graphics = false;
    let mut supports_transfer = false;
    if let Some(get_queue_family_properties) = device_info
        .instance_dispatch
        .get_physical_device_queue_family_properties
    {
        let mut queue_family_count = 0;
        get_queue_family_properties(
            device_info.physical_device,
            &mut queue_family_count,
            ptr::null_mut(),
        );
        let mut queue_families =
            vec![vk::QueueFamilyProperties::default(); queue_family_count as usize];
        get_queue_family_properties(
            device_info.physical_device,
            &mut queue_family_count,
            queue_families.as_mut_ptr(),
        );
        if let Some(queue_family) = queue_families.get(queue_family_index as usize) {
            supports_graphics = queue_family.queue_flags.contains(vk::QueueFlags::GRAPHICS);
            supports_transfer = queue_family.queue_flags.contains(vk::QueueFlags::TRANSFER);
        }
    }
    remember_queue(
        *queue,
        device,
        queue_family_index,
        queue_index,
        supports_graphics,
        supports_transfer,
    );
}

unsafe extern "system" fn layer_get_device_queue2(
    device: vk::Device,
    queue_info: *const DeviceQueueInfo2,
    queue: *mut vk::Queue,
) {
    let device_info = {
        let state = global_state().lock().expect("global state mutex poisoned");
        state
            .device_map
            .get(&dispatch_key_from_handle(device))
            .copied()
            .unwrap_or_default()
    };
    let Some(get_device_queue2) = device_info.dispatch.get_device_queue2 else {
        return;
    };
    get_device_queue2(device, queue_info, queue);

    let queue_info = *queue_info;
    let mut supports_graphics = false;
    let mut supports_transfer = false;
    if let Some(get_queue_family_properties) = device_info
        .instance_dispatch
        .get_physical_device_queue_family_properties
    {
        let mut queue_family_count = 0;
        get_queue_family_properties(
            device_info.physical_device,
            &mut queue_family_count,
            ptr::null_mut(),
        );
        let mut queue_families =
            vec![vk::QueueFamilyProperties::default(); queue_family_count as usize];
        get_queue_family_properties(
            device_info.physical_device,
            &mut queue_family_count,
            queue_families.as_mut_ptr(),
        );
        if let Some(queue_family) = queue_families.get(queue_info.queue_family_index as usize) {
            supports_graphics = queue_family.queue_flags.contains(vk::QueueFlags::GRAPHICS);
            supports_transfer = queue_family.queue_flags.contains(vk::QueueFlags::TRANSFER);
        }
    }
    remember_queue(
        *queue,
        device,
        queue_info.queue_family_index,
        queue_info.queue_index,
        supports_graphics,
        supports_transfer,
    );
}

unsafe extern "system" fn layer_create_swapchain_khr(
    device: vk::Device,
    create_info: *const SwapchainCreateInfoKHR,
    allocator: *const AllocationCallbacks,
    swapchain: *mut vk::SwapchainKHR,
) -> vk::Result {
    let device_info = {
        let state = global_state().lock().expect("global state mutex poisoned");
        state
            .device_map
            .get(&dispatch_key_from_handle(device))
            .copied()
    };
    let Some(device_info) = device_info else {
        log_warn("vkCreateSwapchainKHR: device not found in layer state; passing through without tracking");
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    };

    let mode = Mode::from_env();
    let create_info_ref = &*create_info;
    let mut modified = *create_info_ref;

    let max_image_count = if let Some(get_surface_capabilities) = device_info
        .instance_dispatch
        .get_physical_device_surface_capabilities_khr
    {
        if !create_info_ref.surface.is_null() && !device_info.physical_device.is_null() {
            let mut caps = vk::SurfaceCapabilitiesKHR::default();
            if get_surface_capabilities(
                device_info.physical_device,
                create_info_ref.surface,
                &mut caps,
            ) == vk::Result::SUCCESS
            {
                Some(caps.max_image_count)
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };
    let mutation = mutate_swapchain(
        mode,
        create_info_ref.min_image_count,
        create_info_ref.image_usage,
        max_image_count,
    );
    modified.min_image_count = mutation.modified_min_image_count;
    modified.image_usage = mutation.modified_usage;

    let Some(create_swapchain) = device_info.dispatch.create_swapchain_khr else {
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    };
    let result = create_swapchain(device, &modified, allocator, swapchain);
    if result != vk::Result::SUCCESS {
        log_warn(format!("vkCreateSwapchainKHR failed: {}", result.as_raw()));
        return result;
    }

    if !create_info_ref.old_swapchain.is_null() {
        let old_swapchain_key = create_info_ref.old_swapchain.as_raw();
        let mut state = global_state().lock().expect("global state mutex poisoned");
        if let Some(mut old_swapchain) = state.swapchains.remove(&old_swapchain_key) {
            destroy_swapchain_resources(&device_info, &mut old_swapchain);
        }
    }

    let mut state_entry = SwapchainState {
        device,
        physical_device: device_info.physical_device,
        surface: create_info_ref.surface,
        handle: *swapchain,
        format: modified.image_format,
        extent: modified.image_extent,
        present_mode: modified.present_mode,
        original_usage: create_info_ref.image_usage,
        modified_usage: modified.image_usage,
        original_min_image_count: create_info_ref.min_image_count,
        modified_min_image_count: modified.min_image_count,
        ..Default::default()
    };
    refresh_swapchain_images(&mut state_entry, &device_info.dispatch);
    if matches!(mode, Mode::HistoryCopyTest | Mode::BlendTest) {
        let _ = ensure_history_image(&mut state_entry, &device_info);
    }

    {
        let mut state = global_state().lock().expect("global state mutex poisoned");
        state
            .swapchains
            .insert((*swapchain).as_raw(), state_entry.clone());
    }

    log_info(format!(
        "vkCreateSwapchainKHR ok; extent={}; format={}; presentMode={}; minImages={}->{}; usage={} -> {}; images={}; mode={}",
        format_extent(modified.image_extent),
        modified.image_format.as_raw(),
        present_mode_name(modified.present_mode),
        create_info_ref.min_image_count,
        modified.min_image_count,
        usage_flags(create_info_ref.image_usage),
        usage_flags(modified.image_usage),
        state_entry.images.len(),
        mode.name(),
    ));

    result
}

unsafe extern "system" fn layer_destroy_swapchain_khr(
    device: vk::Device,
    swapchain: vk::SwapchainKHR,
    allocator: *const AllocationCallbacks,
) {
    let device_info = {
        let state = global_state().lock().expect("global state mutex poisoned");
        state
            .device_map
            .get(&dispatch_key_from_handle(device))
            .copied()
    };
    if let Some(device_info) = device_info {
        let mut state = global_state().lock().expect("global state mutex poisoned");
        if let Some(mut swapchain_state) = state.swapchains.remove(&swapchain.as_raw()) {
            destroy_swapchain_resources(&device_info, &mut swapchain_state);
        }
        log_info("vkDestroySwapchainKHR");
        if let Some(destroy_swapchain) = device_info.dispatch.destroy_swapchain_khr {
            destroy_swapchain(device, swapchain, allocator);
        }
    }
}

unsafe extern "system" fn layer_queue_present_khr(
    queue: vk::Queue,
    present_info: *const PresentInfoKHR,
) -> vk::Result {
    let queue_key = queue_id(queue);
    let (queue_info, have_queue, device_info) = {
        let state = global_state().lock().expect("global state mutex poisoned");
        if let Some(queue_info) = state.queue_map.get(&queue_key) {
            let device_info = state
                .device_map
                .get(&dispatch_key_from_handle(queue_info.device))
                .copied()
                .unwrap_or_default();
            (*queue_info, true, device_info)
        } else {
            (QueueInfo::default(), false, DeviceInfo::default())
        }
    };

    let Some(queue_present) = device_info.dispatch.queue_present_khr else {
        log_warn("vkQueuePresentKHR: device dispatch not available");
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    };
    if present_info.is_null() {
        log_warn("vkQueuePresentKHR: present_info was null");
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    }
    if !have_queue {
        log_warn("vkQueuePresentKHR: queue family not tracked; using passthrough-only fallback for this queue");
    }

    let mode = Mode::from_env();
    let present_info_ref = &*present_info;
    if present_info_ref.swapchain_count == 1 {
        let mut state_guard = global_state().lock().expect("global state mutex poisoned");
        if let Some(swapchain_state) = state_guard
            .swapchains
            .get_mut(&(*present_info_ref.p_swapchains).as_raw())
        {
            swapchain_state.present_count += 1;
            if swapchain_state.present_count <= 5 || swapchain_state.present_count % 60 == 0 {
                let prefix = if matches!(
                    mode,
                    Mode::ClearTest | Mode::CopyTest | Mode::HistoryCopyTest | Mode::BlendTest
                ) {
                    "vkQueuePresentKHR frame="
                } else {
                    "vkQueuePresentKHR passthrough frame="
                };
                log_info(format!(
                    "{}{}; queueFamily={}; imageIndex={}; waitSemaphores={}",
                    prefix,
                    swapchain_state.present_count,
                    queue_info.family_index,
                    *present_info_ref.p_image_indices,
                    present_info_ref.wait_semaphore_count
                ));
            }

            match planned_sequence(
                mode,
                &planner::SimulatedPresentState {
                    history_valid: swapchain_state.history_valid,
                    injection_works: swapchain_state.injection_works,
                    generated_present_count: swapchain_state.generated_present_count,
                },
            ) {
                planner::PresentSequence::PassThrough => {}
                planner::PresentSequence::OriginalThenGenerated
                    if matches!(mode, Mode::ClearTest) && have_queue =>
                {
                    drop(state_guard);
                    let original_result = queue_present(queue, present_info);
                    if original_result != vk::Result::SUCCESS
                        && original_result != vk::Result::SUBOPTIMAL_KHR
                    {
                        return original_result;
                    }
                    let mut state_guard =
                        global_state().lock().expect("global state mutex poisoned");
                    if let Some(swapchain_state) = state_guard
                        .swapchains
                        .get_mut(&(*present_info_ref.p_swapchains).as_raw())
                    {
                        swapchain_state.injection_attempted = true;
                        let success = try_present_clear_frame(
                            swapchain_state,
                            &device_info,
                            &queue_info,
                            queue,
                        );
                        let mut sim = planner::SimulatedPresentState {
                            history_valid: swapchain_state.history_valid,
                            injection_works: swapchain_state.injection_works,
                            generated_present_count: swapchain_state.generated_present_count,
                        };
                        mark_injection_result(mode, &mut sim, success);
                    }
                    return original_result;
                }
                planner::PresentSequence::OriginalThenGenerated
                    if matches!(mode, Mode::CopyTest) && have_queue =>
                {
                    swapchain_state.injection_attempted = true;
                    if try_present_copy_frame(
                        swapchain_state,
                        &device_info,
                        &queue_info,
                        queue,
                        present_info,
                    ) {
                        return vk::Result::SUCCESS;
                    }
                }
                planner::PresentSequence::PrimeHistory
                | planner::PresentSequence::GeneratedThenOriginal
                    if matches!(mode, Mode::HistoryCopyTest) && have_queue =>
                {
                    swapchain_state.injection_attempted = true;
                    if try_present_history_copy_frame(
                        swapchain_state,
                        &device_info,
                        &queue_info,
                        queue,
                        present_info,
                    ) {
                        return vk::Result::SUCCESS;
                    }
                }
                planner::PresentSequence::PrimeHistory
                | planner::PresentSequence::GeneratedThenOriginal
                    if matches!(mode, Mode::BlendTest) && have_queue =>
                {
                    swapchain_state.injection_attempted = true;
                    if try_present_blend_frame(
                        swapchain_state,
                        &device_info,
                        &queue_info,
                        queue,
                        present_info,
                    ) {
                        return vk::Result::SUCCESS;
                    }
                }
                _ => {}
            }
        }
    }

    queue_present(queue, present_info)
}

unsafe fn get_instance_fallback_proc_addr(
    instance: vk::Instance,
    name: *const c_char,
) -> vk::PFN_vkVoidFunction {
    let state = global_state().lock().expect("global state mutex poisoned");
    state
        .instance_dispatch
        .get(&dispatch_key_from_handle(instance))
        .and_then(|dispatch| dispatch.get_instance_proc_addr)
        .map(|gipa| gipa(instance, name))
        .unwrap_or(None)
}

unsafe fn get_device_fallback_proc_addr(
    device: vk::Device,
    name: *const c_char,
) -> vk::PFN_vkVoidFunction {
    let state = global_state().lock().expect("global state mutex poisoned");
    state
        .device_map
        .get(&dispatch_key_from_handle(device))
        .and_then(|device_info| device_info.dispatch.get_device_proc_addr)
        .map(|gdpa| gdpa(device, name))
        .unwrap_or(None)
}

unsafe fn pfn_of<T>(func: T) -> vk::PFN_vkVoidFunction {
    mem::transmute_copy(&func)
}

fn write_c_char_array(dest: &mut [c_char], value: &CStr) {
    unsafe {
        ptr::write_bytes(dest.as_mut_ptr(), 0, dest.len());
        let bytes = value.to_bytes_with_nul();
        ptr::copy_nonoverlapping(
            bytes.as_ptr().cast::<c_char>(),
            dest.as_mut_ptr(),
            bytes.len().min(dest.len()),
        );
    }
}

#[no_mangle]
pub unsafe extern "system" fn vkEnumerateInstanceLayerProperties(
    property_count: *mut u32,
    properties: *mut vk::LayerProperties,
) -> vk::Result {
    if property_count.is_null() {
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    }
    *property_count = 1;
    if !properties.is_null() {
        let mut property = vk::LayerProperties::default();
        write_c_char_array(&mut property.layer_name, layer_name());
        write_c_char_array(&mut property.description, layer_description());
        property.implementation_version = 1;
        property.spec_version = vk::make_api_version(0, 1, 3, 250);
        *properties = property;
    }
    vk::Result::SUCCESS
}

#[no_mangle]
pub unsafe extern "system" fn vkEnumerateInstanceExtensionProperties(
    layer_name_ptr: *const c_char,
    property_count: *mut u32,
    _properties: *mut vk::ExtensionProperties,
) -> vk::Result {
    if !layer_name_ptr.is_null() && CStr::from_ptr(layer_name_ptr) != layer_name() {
        return vk::Result::ERROR_LAYER_NOT_PRESENT;
    }
    if !property_count.is_null() {
        *property_count = 0;
    }
    vk::Result::SUCCESS
}

#[no_mangle]
pub unsafe extern "system" fn vkEnumerateDeviceLayerProperties(
    _physical_device: vk::PhysicalDevice,
    property_count: *mut u32,
    properties: *mut vk::LayerProperties,
) -> vk::Result {
    vkEnumerateInstanceLayerProperties(property_count, properties)
}

#[no_mangle]
pub unsafe extern "system" fn vkEnumerateDeviceExtensionProperties(
    physical_device: vk::PhysicalDevice,
    layer_name_ptr: *const c_char,
    property_count: *mut u32,
    properties: *mut vk::ExtensionProperties,
) -> vk::Result {
    if !layer_name_ptr.is_null() && CStr::from_ptr(layer_name_ptr) == layer_name() {
        if !property_count.is_null() {
            *property_count = 0;
        }
        return vk::Result::SUCCESS;
    }

    let state = global_state().lock().expect("global state mutex poisoned");
    let key = dispatch_key_from_handle(physical_device);
    if let Some(dispatch) = state.instance_dispatch.get(&key) {
        if let Some(enumerate_device_extension_properties) =
            dispatch.enumerate_device_extension_properties
        {
            return enumerate_device_extension_properties(
                physical_device,
                layer_name_ptr,
                property_count,
                properties,
            );
        }
    }
    if !property_count.is_null() {
        *property_count = 0;
    }
    vk::Result::SUCCESS
}

#[no_mangle]
pub unsafe extern "system" fn vkGetInstanceProcAddr(
    instance: vk::Instance,
    name: *const c_char,
) -> vk::PFN_vkVoidFunction {
    if name.is_null() {
        return None;
    }
    let name = CStr::from_ptr(name);

    match name.to_bytes() {
        b"vkGetInstanceProcAddr" => pfn_of(
            vkGetInstanceProcAddr
                as unsafe extern "system" fn(vk::Instance, *const c_char) -> vk::PFN_vkVoidFunction,
        ),
        b"vkGetDeviceProcAddr" => pfn_of(
            vkGetDeviceProcAddr
                as unsafe extern "system" fn(vk::Device, *const c_char) -> vk::PFN_vkVoidFunction,
        ),
        b"vkCreateInstance" => pfn_of(layer_create_instance as PfnVkCreateInstance),
        b"vkDestroyInstance" => pfn_of(layer_destroy_instance as PfnVkDestroyInstance),
        b"vkCreateDevice" => pfn_of(layer_create_device as PfnVkCreateDevice),
        b"vkDestroyDevice" => pfn_of(layer_destroy_device as PfnVkDestroyDevice),
        b"vkGetDeviceQueue" => pfn_of(layer_get_device_queue as PfnVkGetDeviceQueue),
        b"vkGetDeviceQueue2" => pfn_of(layer_get_device_queue2 as PfnVkGetDeviceQueue2),
        b"vkCreateSwapchainKHR" => pfn_of(layer_create_swapchain_khr as PfnVkCreateSwapchainKHR),
        b"vkDestroySwapchainKHR" => pfn_of(layer_destroy_swapchain_khr as PfnVkDestroySwapchainKHR),
        b"vkQueuePresentKHR" => pfn_of(layer_queue_present_khr as PfnVkQueuePresentKHR),
        b"vkEnumerateInstanceLayerProperties" => pfn_of(
            vkEnumerateInstanceLayerProperties
                as unsafe extern "system" fn(*mut u32, *mut vk::LayerProperties) -> vk::Result,
        ),
        b"vkEnumerateInstanceExtensionProperties" => pfn_of(
            vkEnumerateInstanceExtensionProperties
                as unsafe extern "system" fn(
                    *const c_char,
                    *mut u32,
                    *mut vk::ExtensionProperties,
                ) -> vk::Result,
        ),
        b"vkEnumerateDeviceLayerProperties" => pfn_of(
            vkEnumerateDeviceLayerProperties
                as unsafe extern "system" fn(
                    vk::PhysicalDevice,
                    *mut u32,
                    *mut vk::LayerProperties,
                ) -> vk::Result,
        ),
        b"vkEnumerateDeviceExtensionProperties" => pfn_of(
            vkEnumerateDeviceExtensionProperties
                as unsafe extern "system" fn(
                    vk::PhysicalDevice,
                    *const c_char,
                    *mut u32,
                    *mut vk::ExtensionProperties,
                ) -> vk::Result,
        ),
        _ => get_instance_fallback_proc_addr(instance, name.as_ptr()),
    }
}

#[no_mangle]
pub unsafe extern "system" fn vkGetDeviceProcAddr(
    device: vk::Device,
    name: *const c_char,
) -> vk::PFN_vkVoidFunction {
    if name.is_null() {
        return None;
    }
    let name = CStr::from_ptr(name);
    match name.to_bytes() {
        b"vkGetDeviceProcAddr" => pfn_of(
            vkGetDeviceProcAddr
                as unsafe extern "system" fn(vk::Device, *const c_char) -> vk::PFN_vkVoidFunction,
        ),
        b"vkGetDeviceQueue" => pfn_of(layer_get_device_queue as PfnVkGetDeviceQueue),
        b"vkGetDeviceQueue2" => pfn_of(layer_get_device_queue2 as PfnVkGetDeviceQueue2),
        b"vkCreateSwapchainKHR" => pfn_of(layer_create_swapchain_khr as PfnVkCreateSwapchainKHR),
        b"vkDestroySwapchainKHR" => pfn_of(layer_destroy_swapchain_khr as PfnVkDestroySwapchainKHR),
        b"vkQueuePresentKHR" => pfn_of(layer_queue_present_khr as PfnVkQueuePresentKHR),
        b"vkDestroyDevice" => pfn_of(layer_destroy_device as PfnVkDestroyDevice),
        _ => get_device_fallback_proc_addr(device, name.as_ptr()),
    }
}

#[no_mangle]
pub unsafe extern "system" fn vkNegotiateLoaderLayerInterfaceVersion(
    version_struct: *mut VkNegotiateLayerInterface,
) -> vk::Result {
    let Some(version_struct) = version_struct.as_mut() else {
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    };
    if version_struct.s_type != VkNegotiateLayerStructType::Interface {
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    }
    if version_struct.loader_layer_interface_version < 2 {
        return vk::Result::ERROR_INITIALIZATION_FAILED;
    }

    version_struct.loader_layer_interface_version = CURRENT_LOADER_LAYER_INTERFACE_VERSION;
    version_struct.pfn_get_instance_proc_addr = vkGetInstanceProcAddr;
    version_struct.pfn_get_device_proc_addr = vkGetDeviceProcAddr;
    version_struct.pfn_get_physical_device_proc_addr = None;

    log_info(format!(
        "vkNegotiateLoaderLayerInterfaceVersion ok; mode={}",
        Mode::from_env().name()
    ));
    vk::Result::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_key_reads_first_pointer_word() {
        let handle_mem = Box::new(0xfeed_beefusize);
        let raw = Box::into_raw(handle_mem) as u64;
        let handle = vk::Instance::from_raw(raw);
        let key = unsafe { dispatch_key_from_handle(handle) };
        assert_eq!(key, 0xfeed_beefusize);
        unsafe {
            drop(Box::from_raw(raw as *mut usize));
        }
    }

    #[test]
    fn enumerates_own_layer_properties() {
        let mut count = 0;
        let result = unsafe { vkEnumerateInstanceLayerProperties(&mut count, ptr::null_mut()) };
        assert_eq!(result, vk::Result::SUCCESS);
        assert_eq!(count, 1);

        let mut prop = vk::LayerProperties::default();
        let result = unsafe { vkEnumerateInstanceLayerProperties(&mut count, &mut prop) };
        assert_eq!(result, vk::Result::SUCCESS);
        let name = unsafe { CStr::from_ptr(prop.layer_name.as_ptr()) };
        assert_eq!(name, layer_name());
    }

    #[test]
    fn proc_addr_returns_known_exports() {
        unsafe {
            assert!(
                vkGetInstanceProcAddr(vk::Instance::null(), cstr!("vkCreateInstance")).is_some()
            );
            assert!(
                vkGetInstanceProcAddr(vk::Instance::null(), cstr!("vkQueuePresentKHR")).is_some()
            );
            assert!(vkGetDeviceProcAddr(vk::Device::null(), cstr!("vkQueuePresentKHR")).is_some());
            assert!(vkGetDeviceProcAddr(vk::Device::null(), cstr!("vkGetDeviceQueue2")).is_some());
        }
    }

    #[test]
    fn negotiates_loader_interface() {
        let mut negotiate = VkNegotiateLayerInterface {
            s_type: VkNegotiateLayerStructType::Interface,
            p_next: ptr::null_mut(),
            loader_layer_interface_version: 2,
            pfn_get_instance_proc_addr: vkGetInstanceProcAddr,
            pfn_get_device_proc_addr: vkGetDeviceProcAddr,
            pfn_get_physical_device_proc_addr: None,
        };
        let result = unsafe { vkNegotiateLoaderLayerInterfaceVersion(&mut negotiate) };
        assert_eq!(result, vk::Result::SUCCESS);
        assert_eq!(negotiate.loader_layer_interface_version, 2);
        let gipa_ptr = negotiate.pfn_get_instance_proc_addr as usize;
        let gdpa_ptr = negotiate.pfn_get_device_proc_addr as usize;
        assert_ne!(gipa_ptr, 0);
        assert_ne!(gdpa_ptr, 0);
    }
}
