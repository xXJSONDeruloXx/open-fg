// GameScopeVK 1:1 compute pipeline port for research.
//
// This module implements the full 20-pass compute dispatch pipeline from
// the GameScopeVK frame interpolation neural network. The decompiled GLSL
// shaders are compiled to SPIR-V and embedded at compile time. The exact
// weight constants and layer structure from the original model are preserved.
//
// The pipeline runs entirely on the GPU via Vulkan compute dispatches:
//   Pass 0:  Pyramid builder       (1 tex → 7 r8 levels)
//   Pass 1:  Feature extract L1    (1 tex → 1 rgba8 feature map)
//   Pass 2:  Feature extract L2    (1 tex → 1 rgba8 feature map)
//   Pass 3-5: Feature channels A-C (1 tex each → 1 rgba8 each)
//   Pass 6:  Flow init             (1 tex → 2 rgba8)
//   Pass 7:  Coarse multi-scale OF (6 tex → 2 rgba8)
//   Pass 8:  OF refinement first   (2 tex → 2 rgba8)
//   Pass 9-11: OF iterative refinement (2 tex → 2 rgba8)
//   Pass 12: Final full-res OF     (2 tex → 2 rgba8)
//   Pass 13: Flow pyramid expand   (2 tex → 6 img)
//   Pass 14: Multi-scale aggregation (6 tex → 1 rgba8)
//   Pass 15: Flow merge 2→1        (2 tex → 1 rgba8)
//   Pass 16: Flow expand 1→2       (1 tex → 2 rgba8)
//   Pass 17: Flow warp + blend     (5 tex + UBO → 3 rgba8)
//   Pass 18: Frame synthesis       (5 tex + UBO → 1 rgba8)

use crate::{DeviceInfo, SwapchainState};
use ash::vk;
use std::ptr;

// Shader SPIR-V blobs embedded at compile time.
const SPV_03: &[u8] = include_bytes!("../shaders/gamescopevk/shader_03.spv");
const SPV_04: &[u8] = include_bytes!("../shaders/gamescopevk/shader_04.spv");
const SPV_05: &[u8] = include_bytes!("../shaders/gamescopevk/shader_05.spv");
const SPV_06: &[u8] = include_bytes!("../shaders/gamescopevk/shader_06.spv");
const SPV_07: &[u8] = include_bytes!("../shaders/gamescopevk/shader_07.spv");
const SPV_08: &[u8] = include_bytes!("../shaders/gamescopevk/shader_08.spv");
const SPV_09: &[u8] = include_bytes!("../shaders/gamescopevk/shader_09.spv");
const SPV_10: &[u8] = include_bytes!("../shaders/gamescopevk/shader_10.spv");
const SPV_11: &[u8] = include_bytes!("../shaders/gamescopevk/shader_11.spv");
const SPV_12: &[u8] = include_bytes!("../shaders/gamescopevk/shader_12.spv");
const SPV_13: &[u8] = include_bytes!("../shaders/gamescopevk/shader_13.spv");
const SPV_14: &[u8] = include_bytes!("../shaders/gamescopevk/shader_14.spv");
const SPV_17: &[u8] = include_bytes!("../shaders/gamescopevk/shader_17.spv");
const SPV_25: &[u8] = include_bytes!("../shaders/gamescopevk/shader_25.spv");
const SPV_26: &[u8] = include_bytes!("../shaders/gamescopevk/shader_26.spv");
const SPV_27: &[u8] = include_bytes!("../shaders/gamescopevk/shader_27.spv");
const SPV_28: &[u8] = include_bytes!("../shaders/gamescopevk/shader_28.spv");
const SPV_29: &[u8] = include_bytes!("../shaders/gamescopevk/shader_29.spv");
const SPV_30: &[u8] = include_bytes!("../shaders/gamescopevk/shader_30.spv");

/// Per-pass SPIR-V blobs in pipeline dispatch order.
const PIPELINE_SPIR_V: [&[u8]; 19] = [
    SPV_03, SPV_05, SPV_06, SPV_26, SPV_27, SPV_28, SPV_07, SPV_09, SPV_08, SPV_10, SPV_11, SPV_12,
    SPV_17, SPV_13, SPV_25, SPV_29, SPV_30, SPV_14, SPV_04,
];

/// UBO layout for shaders that take (scale, alpha/t, epsilon).
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct GsVkUbo {
    pub scale: f32,
    pub alpha: f32,
    pub epsilon: f32,
}

/// An allocated image + memory + view triple.
#[derive(Clone, Copy, Default)]
struct ImageResource {
    image: vk::Image,
    memory: vk::DeviceMemory,
    view: vk::ImageView,
}

/// All managed resources for the GS-VK compute pipeline.
pub struct GsVkPipeline {
    pipelines: [vk::Pipeline; 19],
    pipeline_layout: vk::PipelineLayout,
    descriptor_set_layout: vk::DescriptorSetLayout,
    descriptor_pool: vk::DescriptorPool,
    /// One descriptor set per pass — each has different images bound at b32..b54.
    descriptor_sets: [vk::DescriptorSet; 19],
    command_pool: vk::CommandPool,
    command_buffer: vk::CommandBuffer,
    fence: vk::Fence,
    sampler: vk::Sampler,
    ubo_buffer: vk::Buffer,
    ubo_memory: vk::DeviceMemory,

    // Intermediate images.
    // Pyramid levels (r8) — shader_03 outputs 7 levels.
    pyramid: [ImageResource; 7],
    // Feature maps (rgba8) — shaders 05, 06, 26, 27, 28.
    features: [ImageResource; 5],
    // Flow fields (rgba8) — forward and backward optical flow.
    flow_fwd: ImageResource,
    flow_bwd: ImageResource,
    // Warp intermediate outputs (rgba8) — shader_14 outputs 3.
    warp: [ImageResource; 3],
    // Output image (rgba8) — final synthesized frame from shader_04.
    output: ImageResource,
    // Temporary ping-pong images for iterative OF refinement.
    temp_a: ImageResource,
    temp_b: ImageResource,

    initialized: bool,
}

impl GsVkPipeline {
    pub const fn new() -> Self {
        const NULL_IMG: ImageResource = ImageResource {
            image: vk::Image::null(),
            memory: vk::DeviceMemory::null(),
            view: vk::ImageView::null(),
        };
        Self {
            pipelines: [vk::Pipeline::null(); 19],
            pipeline_layout: vk::PipelineLayout::null(),
            descriptor_set_layout: vk::DescriptorSetLayout::null(),
            descriptor_pool: vk::DescriptorPool::null(),
            descriptor_sets: [vk::DescriptorSet::null(); 19],
            command_pool: vk::CommandPool::null(),
            command_buffer: vk::CommandBuffer::null(),
            fence: vk::Fence::null(),
            sampler: vk::Sampler::null(),
            ubo_buffer: vk::Buffer::null(),
            ubo_memory: vk::DeviceMemory::null(),
            pyramid: [NULL_IMG; 7],
            features: [NULL_IMG; 5],
            flow_fwd: NULL_IMG,
            flow_bwd: NULL_IMG,
            warp: [NULL_IMG; 3],
            output: NULL_IMG,
            temp_a: NULL_IMG,
            temp_b: NULL_IMG,
            initialized: false,
        }
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized
    }
}

/// Helper to create a storage image (r8 or rgba8) and allocate+bind memory+view.
unsafe fn create_storage_image(
    device: vk::Device,
    format: vk::Format,
    extent: vk::Extent2D,
    dispatch: &crate::DeviceDispatch,
    mem_props: vk::PhysicalDeviceMemoryProperties,
) -> Option<ImageResource> {
    let usage = vk::ImageUsageFlags::STORAGE | vk::ImageUsageFlags::SAMPLED;
    let create_info = vk::ImageCreateInfo {
        s_type: vk::StructureType::IMAGE_CREATE_INFO,
        image_type: vk::ImageType::TYPE_2D,
        format,
        extent: vk::Extent3D {
            width: extent.width,
            height: extent.height,
            depth: 1,
        },
        mip_levels: 1,
        array_layers: 1,
        samples: vk::SampleCountFlags::TYPE_1,
        tiling: vk::ImageTiling::OPTIMAL,
        usage,
        sharing_mode: vk::SharingMode::EXCLUSIVE,
        initial_layout: vk::ImageLayout::UNDEFINED,
        ..Default::default()
    };

    let mut image = vk::Image::null();
    let create_image = dispatch.create_image?;
    if create_image(device, &create_info, ptr::null(), &mut image) != vk::Result::SUCCESS {
        return None;
    }

    let get_reqs = dispatch.get_image_memory_requirements?;
    let alloc_mem = dispatch.allocate_memory?;
    let bind_img = dispatch.bind_image_memory?;
    let create_view = dispatch.create_image_view?;

    let mut reqs = vk::MemoryRequirements::default();
    get_reqs(device, image, &mut reqs);

    let memory_type_index = find_memory_type(
        mem_props,
        reqs.memory_type_bits,
        vk::MemoryPropertyFlags::DEVICE_LOCAL,
    )?;

    let alloc_info = vk::MemoryAllocateInfo {
        s_type: vk::StructureType::MEMORY_ALLOCATE_INFO,
        allocation_size: reqs.size,
        memory_type_index,
        ..Default::default()
    };
    let mut memory = vk::DeviceMemory::null();
    if alloc_mem(device, &alloc_info, ptr::null(), &mut memory) != vk::Result::SUCCESS {
        let destroy_image = dispatch.destroy_image.unwrap();
        destroy_image(device, image, ptr::null());
        return None;
    }
    if bind_img(device, image, memory, 0) != vk::Result::SUCCESS {
        let free_mem = dispatch.free_memory.unwrap();
        let destroy_image = dispatch.destroy_image.unwrap();
        free_mem(device, memory, ptr::null());
        destroy_image(device, image, ptr::null());
        return None;
    }

    let subresource = vk::ImageSubresourceRange {
        aspect_mask: vk::ImageAspectFlags::COLOR,
        base_mip_level: 0,
        level_count: 1,
        base_array_layer: 0,
        layer_count: 1,
    };
    let view_info = vk::ImageViewCreateInfo {
        s_type: vk::StructureType::IMAGE_VIEW_CREATE_INFO,
        image,
        view_type: vk::ImageViewType::TYPE_2D,
        format,
        subresource_range: subresource,
        ..Default::default()
    };
    let mut view = vk::ImageView::null();
    if create_view(device, &view_info, ptr::null(), &mut view) != vk::Result::SUCCESS {
        let free_mem = dispatch.free_memory.unwrap();
        let destroy_image = dispatch.destroy_image.unwrap();
        free_mem(device, memory, ptr::null());
        destroy_image(device, image, ptr::null());
        return None;
    }

    Some(ImageResource {
        image,
        memory,
        view,
    })
}

/// Helper to create a UBO buffer.
unsafe fn create_ubo_buffer(
    device: vk::Device,
    size: vk::DeviceSize,
    dispatch: &crate::DeviceDispatch,
    mem_props: vk::PhysicalDeviceMemoryProperties,
) -> Option<(vk::Buffer, vk::DeviceMemory)> {
    let create_buffer = dispatch.create_buffer?;
    let get_buf_reqs = dispatch.get_buffer_memory_requirements?;
    let alloc_mem = dispatch.allocate_memory?;
    let bind_buf = dispatch.bind_buffer_memory?;

    let buf_info = vk::BufferCreateInfo {
        s_type: vk::StructureType::BUFFER_CREATE_INFO,
        size,
        usage: vk::BufferUsageFlags::UNIFORM_BUFFER,
        sharing_mode: vk::SharingMode::EXCLUSIVE,
        ..Default::default()
    };
    let mut buffer = vk::Buffer::null();
    if create_buffer(device, &buf_info, ptr::null(), &mut buffer) != vk::Result::SUCCESS {
        return None;
    }

    let mut reqs = vk::MemoryRequirements::default();
    get_buf_reqs(device, buffer, &mut reqs);

    let memory_type_index = find_memory_type(
        mem_props,
        reqs.memory_type_bits,
        vk::MemoryPropertyFlags::HOST_VISIBLE | vk::MemoryPropertyFlags::HOST_COHERENT,
    )?;

    let alloc_info = vk::MemoryAllocateInfo {
        s_type: vk::StructureType::MEMORY_ALLOCATE_INFO,
        allocation_size: reqs.size,
        memory_type_index,
        ..Default::default()
    };
    let mut memory = vk::DeviceMemory::null();
    if alloc_mem(device, &alloc_info, ptr::null(), &mut memory) != vk::Result::SUCCESS {
        let destroy_buf = dispatch.destroy_buffer.unwrap();
        destroy_buf(device, buffer, ptr::null());
        return None;
    }
    if bind_buf(device, buffer, memory, 0) != vk::Result::SUCCESS {
        let free_mem = dispatch.free_memory.unwrap();
        let destroy_buf = dispatch.destroy_buffer.unwrap();
        free_mem(device, memory, ptr::null());
        destroy_buf(device, buffer, ptr::null());
        return None;
    }

    Some((buffer, memory))
}

fn find_memory_type(
    mem_props: vk::PhysicalDeviceMemoryProperties,
    type_bits: u32,
    flags: vk::MemoryPropertyFlags,
) -> Option<u32> {
    for i in 0..mem_props.memory_type_count {
        if (type_bits & (1 << i)) != 0
            && mem_props.memory_types[i as usize]
                .property_flags
                .contains(flags)
        {
            return Some(i);
        }
    }
    None
}

/// Initialize the full GS-VK compute pipeline.
///
/// Returns true if all 19 compute pipelines, intermediate images, and
/// descriptor sets were created successfully.
pub unsafe fn init_gs_vk_pipeline(
    swapchain: &mut SwapchainState,
    device_info: &DeviceInfo,
) -> bool {
    if swapchain.gs_vk.is_initialized() {
        return true;
    }

    // Extract all required function pointers.
    let (
        Some(create_compute_pipelines),
        Some(create_descriptor_set_layout),
        Some(create_descriptor_pool),
        Some(allocate_descriptor_sets),
        Some(create_pipeline_layout),
        Some(create_shader_module),
        Some(destroy_shader_module),
        Some(create_command_pool),
        Some(allocate_command_buffers),
        Some(create_fence),
        Some(create_sampler),
        Some(create_image),
        Some(get_image_memory_requirements),
        Some(allocate_memory),
        Some(bind_image_memory),
        Some(create_image_view),
        Some(create_buffer),
        Some(get_buffer_memory_requirements),
        Some(bind_buffer_memory),
        Some(update_descriptor_sets),
    ) = (
        device_info.dispatch.create_compute_pipelines,
        device_info.dispatch.create_descriptor_set_layout,
        device_info.dispatch.create_descriptor_pool,
        device_info.dispatch.allocate_descriptor_sets,
        device_info.dispatch.create_pipeline_layout,
        device_info.dispatch.create_shader_module,
        device_info.dispatch.destroy_shader_module,
        device_info.dispatch.create_command_pool,
        device_info.dispatch.allocate_command_buffers,
        device_info.dispatch.create_fence,
        device_info.dispatch.create_sampler,
        device_info.dispatch.create_image,
        device_info.dispatch.get_image_memory_requirements,
        device_info.dispatch.allocate_memory,
        device_info.dispatch.bind_image_memory,
        device_info.dispatch.create_image_view,
        device_info.dispatch.create_buffer,
        device_info.dispatch.get_buffer_memory_requirements,
        device_info.dispatch.bind_buffer_memory,
        device_info.dispatch.update_descriptor_sets,
    )
    else {
        crate::log_warn("gs-vk: required device functions unavailable");
        return false;
    };

    let device = device_info.device;
    let extent = swapchain.extent;
    let mem_props = device_info.memory_properties;
    let dispatch = &device_info.dispatch;

    // Descriptor set layout: UBO at b0, sampler at b31, images at b32-b54, storage images at b48-b54.
    // The GameScopeVK shaders use bindings:
    //   b0: UBO (std140, 3 floats)
    //   b31: sampler (for texture reads)
    //   b32-b47: combined image samplers (source frames, feature maps, flow fields)
    //   b48-b54: storage images (output images)
    let mut bindings = Vec::new();

    // UBO binding
    bindings.push(vk::DescriptorSetLayoutBinding {
        binding: 0,
        descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
        descriptor_count: 1,
        stage_flags: vk::ShaderStageFlags::COMPUTE,
        ..Default::default()
    });

    // Sampler binding
    bindings.push(vk::DescriptorSetLayoutBinding {
        binding: 31,
        descriptor_type: vk::DescriptorType::SAMPLER,
        descriptor_count: 1,
        stage_flags: vk::ShaderStageFlags::COMPUTE,
        ..Default::default()
    });

    // Combined image samplers b32..=b47 (up to 16 input textures)
    for b in 32u32..=47u32 {
        bindings.push(vk::DescriptorSetLayoutBinding {
            binding: b,
            descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            descriptor_count: 1,
            stage_flags: vk::ShaderStageFlags::COMPUTE,
            ..Default::default()
        });
    }

    // Storage images b48..=b54 (up to 7 output images)
    for b in 48u32..=54u32 {
        bindings.push(vk::DescriptorSetLayoutBinding {
            binding: b,
            descriptor_type: vk::DescriptorType::STORAGE_IMAGE,
            descriptor_count: 1,
            stage_flags: vk::ShaderStageFlags::COMPUTE,
            ..Default::default()
        });
    }

    let ds_layout_info = vk::DescriptorSetLayoutCreateInfo {
        s_type: vk::StructureType::DESCRIPTOR_SET_LAYOUT_CREATE_INFO,
        binding_count: bindings.len() as u32,
        p_bindings: bindings.as_ptr(),
        ..Default::default()
    };
    let mut ds_layout = vk::DescriptorSetLayout::null();
    if create_descriptor_set_layout(device, &ds_layout_info, ptr::null(), &mut ds_layout)
        != vk::Result::SUCCESS
    {
        crate::log_warn("gs-vk: CreateDescriptorSetLayout failed");
        return false;
    }

    // Descriptor pool — large enough for all the bindings.
    let pool_sizes = [
        vk::DescriptorPoolSize {
            ty: vk::DescriptorType::UNIFORM_BUFFER,
            descriptor_count: 19,
        },
        vk::DescriptorPoolSize {
            ty: vk::DescriptorType::SAMPLER,
            descriptor_count: 19,
        },
        vk::DescriptorPoolSize {
            ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            descriptor_count: 19 * 16,
        },
        vk::DescriptorPoolSize {
            ty: vk::DescriptorType::STORAGE_IMAGE,
            descriptor_count: 19 * 7,
        },
    ];
    let pool_info = vk::DescriptorPoolCreateInfo {
        s_type: vk::StructureType::DESCRIPTOR_POOL_CREATE_INFO,
        max_sets: 19,
        pool_size_count: pool_sizes.len() as u32,
        p_pool_sizes: pool_sizes.as_ptr(),
        ..Default::default()
    };
    let mut pool = vk::DescriptorPool::null();
    if create_descriptor_pool(device, &pool_info, ptr::null(), &mut pool) != vk::Result::SUCCESS {
        crate::log_warn("gs-vk: CreateDescriptorPool failed");
        let destroy_dsl = device_info.dispatch.destroy_descriptor_set_layout.unwrap();
        destroy_dsl(device, ds_layout, ptr::null());
        return false;
    }

    // Allocate 19 descriptor sets — one per pass.
    let mut ds_array = [vk::DescriptorSet::null(); 19];
    let ds_layouts = [ds_layout; 19];
    let ds_alloc = vk::DescriptorSetAllocateInfo {
        s_type: vk::StructureType::DESCRIPTOR_SET_ALLOCATE_INFO,
        descriptor_pool: pool,
        descriptor_set_count: 19,
        p_set_layouts: ds_layouts.as_ptr(),
        ..Default::default()
    };
    if allocate_descriptor_sets(device, &ds_alloc, ds_array.as_mut_ptr()) != vk::Result::SUCCESS {
        crate::log_warn("gs-vk: AllocateDescriptorSets failed");
        let destroy_pool = device_info.dispatch.destroy_descriptor_pool.unwrap();
        let destroy_dsl = device_info.dispatch.destroy_descriptor_set_layout.unwrap();
        destroy_pool(device, pool, ptr::null());
        destroy_dsl(device, ds_layout, ptr::null());
        return false;
    }

    // Pipeline layout.
    let set_layouts = [ds_layout];
    let pc_range = vk::PushConstantRange {
        stage_flags: vk::ShaderStageFlags::COMPUTE,
        offset: 0,
        size: std::mem::size_of::<GsVkUbo>() as u32,
    };
    let pl_info = vk::PipelineLayoutCreateInfo {
        s_type: vk::StructureType::PIPELINE_LAYOUT_CREATE_INFO,
        set_layout_count: set_layouts.len() as u32,
        p_set_layouts: set_layouts.as_ptr(),
        push_constant_range_count: 1,
        p_push_constant_ranges: &pc_range,
        ..Default::default()
    };
    let mut pipeline_layout = vk::PipelineLayout::null();
    if create_pipeline_layout(device, &pl_info, ptr::null(), &mut pipeline_layout)
        != vk::Result::SUCCESS
    {
        crate::log_warn("gs-vk: CreatePipelineLayout failed");
        let destroy_pool = device_info.dispatch.destroy_descriptor_pool.unwrap();
        let destroy_dsl = device_info.dispatch.destroy_descriptor_set_layout.unwrap();
        destroy_pool(device, pool, ptr::null());
        destroy_dsl(device, ds_layout, ptr::null());
        return false;
    }

    // Create 19 compute pipelines from SPIR-V blobs.
    let mut pipelines = [vk::Pipeline::null(); 19];
    let shader_entry = b"main\0".as_ptr().cast::<std::os::raw::c_char>();
    let mut shader_modules = Vec::new();

    for (i, spv) in PIPELINE_SPIR_V.iter().enumerate() {
        let module_info = vk::ShaderModuleCreateInfo {
            s_type: vk::StructureType::SHADER_MODULE_CREATE_INFO,
            code_size: spv.len(),
            p_code: spv.as_ptr() as *const u32,
            ..Default::default()
        };
        let mut module = vk::ShaderModule::null();
        if create_shader_module(device, &module_info, ptr::null(), &mut module)
            != vk::Result::SUCCESS
        {
            crate::log_warn(format!("gs-vk: CreateShaderModule failed for pipeline {i}"));
            // Clean up already-created modules
            for &m in &shader_modules {
                destroy_shader_module(device, m, ptr::null());
            }
            // Clean up already-created pipelines
            let destroy_pipeline = device_info.dispatch.destroy_pipeline.unwrap();
            for &p in &pipelines[..i] {
                if p != vk::Pipeline::null() {
                    destroy_pipeline(device, p, ptr::null());
                }
            }
            return false;
        }
        shader_modules.push(module);

        let stage = vk::PipelineShaderStageCreateInfo {
            s_type: vk::StructureType::PIPELINE_SHADER_STAGE_CREATE_INFO,
            stage: vk::ShaderStageFlags::COMPUTE,
            module,
            p_name: shader_entry,
            ..Default::default()
        };
        let comp_info = vk::ComputePipelineCreateInfo {
            s_type: vk::StructureType::COMPUTE_PIPELINE_CREATE_INFO,
            stage,
            layout: pipeline_layout,
            ..Default::default()
        };
        if create_compute_pipelines(
            device,
            vk::PipelineCache::null(),
            1,
            &comp_info,
            ptr::null(),
            &mut pipelines[i],
        ) != vk::Result::SUCCESS
        {
            crate::log_warn(format!(
                "gs-vk: CreateComputePipelines failed for pipeline {i}"
            ));
            for &m in &shader_modules {
                destroy_shader_module(device, m, ptr::null());
            }
            let destroy_pipeline = device_info.dispatch.destroy_pipeline.unwrap();
            for &p in &pipelines[..i] {
                if p != vk::Pipeline::null() {
                    destroy_pipeline(device, p, ptr::null());
                }
            }
            return false;
        }
    }

    // Shader modules can be destroyed after pipeline creation.
    for &m in &shader_modules {
        destroy_shader_module(device, m, ptr::null());
    }

    // Create sampler.
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
        max_lod: vk::LOD_CLAMP_NONE,
        border_color: vk::BorderColor::FLOAT_OPAQUE_BLACK,
        unnormalized_coordinates: vk::FALSE,
        ..Default::default()
    };
    let mut sampler = vk::Sampler::null();
    if create_sampler(device, &sampler_info, ptr::null(), &mut sampler) != vk::Result::SUCCESS {
        crate::log_warn("gs-vk: CreateSampler failed");
        let destroy_pipeline = device_info.dispatch.destroy_pipeline.unwrap();
        for &p in &pipelines {
            destroy_pipeline(device, p, ptr::null());
        }
        return false;
    }

    // Create UBO buffer.
    let Some((ubo_buffer, ubo_memory)) = create_ubo_buffer(
        device,
        std::mem::size_of::<GsVkUbo>() as vk::DeviceSize,
        dispatch,
        mem_props,
    ) else {
        crate::log_warn("gs-vk: UBO buffer creation failed");
        let destroy_pipeline = device_info.dispatch.destroy_pipeline.unwrap();
        for &p in &pipelines {
            destroy_pipeline(device, p, ptr::null());
        }
        return false;
    };

    // Create intermediate images.
    // Pyramid levels (r8) at decreasing resolutions.
    let r8 = vk::Format::R8_UNORM;
    let rgba8 = vk::Format::R8G8B8A8_UNORM;
    let w = extent.width;
    let h = extent.height;

    let pyramid_resolutions: [(u32, u32); 7] = [
        (w, h),
        (w / 2, h / 2),
        (w / 4, h / 4),
        (w / 8, h / 8),
        (w / 16, h / 16),
        (w / 32, h / 32),
        (w / 64, h / 64),
    ];

    let mut pyramid = [ImageResource::default(); 7];
    for (i, &(pw, ph)) in pyramid_resolutions.iter().enumerate() {
        let Some(img) = create_storage_image(
            device,
            r8,
            vk::Extent2D {
                width: pw.max(1),
                height: ph.max(1),
            },
            dispatch,
            mem_props,
        ) else {
            crate::log_warn(format!("gs-vk: pyramid image {i} creation failed"));
            return false;
        };
        pyramid[i] = img;
    }

    // Feature maps (rgba8, full resolution).
    let mut features = [ImageResource::default(); 5];
    for (i, f) in features.iter_mut().enumerate() {
        let Some(img) = create_storage_image(device, rgba8, extent, dispatch, mem_props) else {
            crate::log_warn(format!("gs-vk: feature image {i} creation failed"));
            return false;
        };
        *f = img;
    }

    // Flow fields (rgba8, full resolution).
    let Some(flow_fwd) = create_storage_image(device, rgba8, extent, dispatch, mem_props) else {
        crate::log_warn("gs-vk: flow_fwd image creation failed");
        return false;
    };
    let Some(flow_bwd) = create_storage_image(device, rgba8, extent, dispatch, mem_props) else {
        crate::log_warn("gs-vk: flow_bwd image creation failed");
        return false;
    };

    // Warp outputs (rgba8, full resolution).
    let mut warp = [ImageResource::default(); 3];
    for (i, w) in warp.iter_mut().enumerate() {
        let Some(img) = create_storage_image(device, rgba8, extent, dispatch, mem_props) else {
            crate::log_warn(format!("gs-vk: warp image {i} creation failed"));
            return false;
        };
        *w = img;
    }

    // Output image (rgba8, full resolution).
    let Some(output) = create_storage_image(device, rgba8, extent, dispatch, mem_props) else {
        crate::log_warn("gs-vk: output image creation failed");
        return false;
    };

    // Temp ping-pong images for OF refinement.
    let Some(temp_a) = create_storage_image(device, rgba8, extent, dispatch, mem_props) else {
        crate::log_warn("gs-vk: temp_a image creation failed");
        return false;
    };
    let Some(temp_b) = create_storage_image(device, rgba8, extent, dispatch, mem_props) else {
        crate::log_warn("gs-vk: temp_b image creation failed");
        return false;
    };

    // Create command pool + buffer.
    let cp_info = vk::CommandPoolCreateInfo {
        s_type: vk::StructureType::COMMAND_POOL_CREATE_INFO,
        queue_family_index: device_info.queue.family_index,
        ..Default::default()
    };
    let mut command_pool = vk::CommandPool::null();
    if create_command_pool(device, &cp_info, ptr::null(), &mut command_pool) != vk::Result::SUCCESS
    {
        crate::log_warn("gs-vk: CreateCommandPool failed");
        return false;
    }

    let cb_info = vk::CommandBufferAllocateInfo {
        s_type: vk::StructureType::COMMAND_BUFFER_ALLOCATE_INFO,
        command_pool,
        level: vk::CommandBufferLevel::PRIMARY,
        command_buffer_count: 1,
        ..Default::default()
    };
    let mut command_buffer = vk::CommandBuffer::null();
    if allocate_command_buffers(device, &cb_info, &mut command_buffer) != vk::Result::SUCCESS {
        crate::log_warn("gs-vk: AllocateCommandBuffers failed");
        return false;
    }

    // Create fence.
    let fence_info = vk::FenceCreateInfo {
        s_type: vk::StructureType::FENCE_CREATE_INFO,
        flags: vk::FenceCreateFlags::SIGNALED,
        ..Default::default()
    };
    let mut fence = vk::Fence::null();
    if create_fence(device, &fence_info, ptr::null(), &mut fence) != vk::Result::SUCCESS {
        crate::log_warn("gs-vk: CreateFence failed");
        return false;
    }

    // Update descriptor set with all the images and sampler.
    let mut writes = Vec::new();
    let mut image_infos = Vec::new();
    let mut buffer_infos = Vec::new();

    // UBO at binding 0
    buffer_infos.push(vk::DescriptorBufferInfo {
        buffer: ubo_buffer,
        offset: 0,
        range: std::mem::size_of::<GsVkUbo>() as vk::DeviceSize,
    });
    writes.push(vk::WriteDescriptorSet {
        s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
        dst_set: ds_array[0],
        dst_binding: 0,
        dst_array_element: 0,
        descriptor_count: 1,
        descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
        p_buffer_info: buffer_infos.last().unwrap(),
        ..Default::default()
    });

    // Sampler at binding 31
    let sampler_info_ds = vk::DescriptorImageInfo {
        sampler,
        image_view: vk::ImageView::null(),
        image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
    };
    image_infos.push(sampler_info_ds);
    writes.push(vk::WriteDescriptorSet {
        s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
        dst_set: ds_array[0],
        dst_binding: 31,
        dst_array_element: 0,
        descriptor_count: 1,
        descriptor_type: vk::DescriptorType::SAMPLER,
        p_image_info: image_infos.last().unwrap(),
        ..Default::default()
    });

    // Combined image samplers for bindings 32..47.
    // We write the source images at b32 (prev frame) and b33 (curr frame).
    // The rest will be filled in per-dispatch dynamically.
    // For now, bind the history and swapchain views.
    // Note: these will need to be re-bound each frame since swapchain images change.
    // The descriptor set update is done once here for static resources.
    // Frame-specific resources (swapchain image views) need to be updated per-frame.

    // Storage images at bindings 48..54 — bind the intermediate images.
    // b48 = output/primary, b49 = secondary, b50 = tertiary, b51..54 = pyramid temps
    let storage_views: [vk::ImageView; 7] = [
        pyramid[0].view, // b48 — first pyramid level (full res)
        pyramid[1].view, // b49
        pyramid[2].view, // b50
        pyramid[3].view, // b51
        pyramid[4].view, // b52
        pyramid[5].view, // b53
        pyramid[6].view, // b54
    ];

    for (i, &view) in storage_views.iter().enumerate() {
        let img_info = vk::DescriptorImageInfo {
            sampler: vk::Sampler::null(),
            image_view: view,
            image_layout: vk::ImageLayout::GENERAL,
        };
        image_infos.push(img_info);
        writes.push(vk::WriteDescriptorSet {
            s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
            dst_set: ds_array[0],
            dst_binding: 48 + i as u32,
            dst_array_element: 0,
            descriptor_count: 1,
            descriptor_type: vk::DescriptorType::STORAGE_IMAGE,
            p_image_info: image_infos.last().unwrap(),
            ..Default::default()
        });
    }

    update_descriptor_sets(device, writes.len() as u32, writes.as_ptr(), 0, ptr::null());

    // Store everything.
    swapchain.gs_vk = GsVkPipeline {
        pipelines,
        pipeline_layout,
        descriptor_set_layout: ds_layout,
        descriptor_pool: pool,
        descriptor_sets: ds_array,
        command_pool,
        command_buffer,
        fence,
        sampler,
        ubo_buffer,
        ubo_memory,
        pyramid,
        features,
        flow_fwd,
        flow_bwd,
        warp,
        output,
        temp_a,
        temp_b,
        initialized: true,
    };

    crate::log_info(format!(
        "gs-vk: initialized 19 compute pipelines, {} intermediate images, extent {}x{}",
        7 + 5 + 2 + 3 + 1 + 2,
        extent.width,
        extent.height
    ));

    true
}

/// Destroy all GS-VK pipeline resources.
pub unsafe fn destroy_gs_vk_pipeline(
    dispatch: &crate::DeviceDispatch,
    device: vk::Device,
    gs_vk: &mut GsVkPipeline,
) {
    if !gs_vk.is_initialized() {
        return;
    }

    let null = ptr::null();

    if let Some(destroy_pipeline) = dispatch.destroy_pipeline {
        for &p in &gs_vk.pipelines {
            if p != vk::Pipeline::null() {
                destroy_pipeline(device, p, null);
            }
        }
    }
    if let Some(f) = dispatch.destroy_pipeline_layout {
        f(device, gs_vk.pipeline_layout, null);
    }
    if let Some(f) = dispatch.destroy_descriptor_set_layout {
        f(device, gs_vk.descriptor_set_layout, null);
    }
    if let Some(f) = dispatch.destroy_descriptor_pool {
        f(device, gs_vk.descriptor_pool, null);
    }
    if let Some(f) = dispatch.destroy_command_pool {
        f(device, gs_vk.command_pool, null);
    }
    if let Some(f) = dispatch.destroy_fence {
        f(device, gs_vk.fence, null);
    }
    if let Some(f) = dispatch.destroy_sampler {
        f(device, gs_vk.sampler, null);
    }
    if let Some(f) = dispatch.destroy_buffer {
        f(device, gs_vk.ubo_buffer, null);
    }
    if let Some(f) = dispatch.free_memory {
        f(device, gs_vk.ubo_memory, null);
    }

    // Destroy all intermediate images.
    let all_images: Vec<&ImageResource> = gs_vk
        .pyramid
        .iter()
        .chain(gs_vk.features.iter())
        .chain([&gs_vk.flow_fwd, &gs_vk.flow_bwd].into_iter())
        .chain(gs_vk.warp.iter())
        .chain([&gs_vk.output, &gs_vk.temp_a, &gs_vk.temp_b].into_iter())
        .collect();

    for img in &all_images {
        if let Some(f) = dispatch.destroy_image_view {
            if img.view != vk::ImageView::null() {
                f(device, img.view, null);
            }
        }
        if let Some(f) = dispatch.destroy_image {
            if img.image != vk::Image::null() {
                f(device, img.image, null);
            }
        }
        if let Some(f) = dispatch.free_memory {
            if img.memory != vk::DeviceMemory::null() {
                f(device, img.memory, null);
            }
        }
    }

    *gs_vk = GsVkPipeline::new();
}

/// Update the UBO with the current frame interpolation parameters.
pub unsafe fn update_gs_vk_ubo(
    device: vk::Device,
    dispatch: &crate::DeviceDispatch,
    gs_vk: &GsVkPipeline,
    alpha: f32,
) {
    let Some(map_memory) = dispatch.map_memory else {
        return;
    };
    let Some(unmap_memory) = dispatch.unmap_memory else {
        return;
    };

    let ubo = GsVkUbo {
        scale: 1.0,
        alpha,
        epsilon: 0.001,
    };

    let mut data: *mut std::ffi::c_void = ptr::null_mut();
    if map_memory(
        device,
        gs_vk.ubo_memory,
        0,
        std::mem::size_of::<GsVkUbo>() as vk::DeviceSize,
        vk::MemoryMapFlags::empty(),
        &mut data,
    ) == vk::Result::SUCCESS
    {
        ptr::copy_nonoverlapping(
            &ubo as *const GsVkUbo as *const u8,
            data as *mut u8,
            std::mem::size_of::<GsVkUbo>(),
        );
        unmap_memory(device, gs_vk.ubo_memory);
    }
}

// ---------------------------------------------------------------------------
// Per-pass workgroup dispatch dimensions
// ---------------------------------------------------------------------------

/// Each shader uses local_size_x=16, local_size_y=16.
/// The dispatch dimensions are ceil(width/16) × ceil(height/16).
/// Some passes operate at reduced resolution.
fn pass_dispatch_extent(pass: usize, frame_w: u32, frame_h: u32) -> (u32, u32) {
    let (w, h) = match pass {
        0 => (frame_w, frame_h),             // pyramid builder — full res
        1..=5 => (frame_w / 2, frame_h / 2), // feature extraction — half res
        6 => (frame_w / 2, frame_h / 2),     // flow init — half res
        7 => (frame_w / 8, frame_h / 8),     // coarse multi-scale OF — 1/8 res
        8 => (frame_w / 2, frame_h / 2),     // OF refinement first — half res
        9 => (frame_w / 4, frame_h / 4),     // OF iterative — 1/4 res
        10 => (frame_w / 2, frame_h / 2),    // OF iterative — half res
        11 => (frame_w, frame_h),            // OF iterative — full res
        12 => (frame_w, frame_h),            // final full-res OF
        13 => (frame_w, frame_h),            // flow pyramid expand
        14 => (frame_w, frame_h),            // multi-scale aggregation
        15 => (frame_w, frame_h),            // flow merge
        16 => (frame_w, frame_h),            // flow expand
        17..=18 => (frame_w, frame_h),       // warp + synthesis — full res
        _ => (frame_w, frame_h),
    };
    let wg = 16u32;
    ((w.max(1) + wg - 1) / wg, (h.max(1) + wg - 1) / wg)
}

// ---------------------------------------------------------------------------
// Per-pass descriptor binding tables
// ---------------------------------------------------------------------------

/// Returns the input texture views (b32..) for a given pass.
/// These are COMBINED_IMAGE_SAMPLER bindings — read-only inputs.
///
/// `history_view` = previous frame (swapchain history image)
/// `swapchain_view` = current frame (swapchain image)
///
/// Intermediate image views come from the GsVkPipeline's allocated resources.
fn pass_input_bindings(
    pass: usize,
    history_view: vk::ImageView,
    swapchain_view: vk::ImageView,
    gs_vk: &GsVkPipeline,
) -> Vec<(u32, vk::ImageView)> {
    match pass {
        // Pass 0: pyramid builder — tex@32 = source frame
        0 => vec![(32, history_view)],

        // Pass 1-2: feature extract — tex@32 = source frame (2× downsampled)
        1 => vec![(32, history_view)],
        2 => vec![(32, history_view)],

        // Pass 3-5: feature channels — tex@32 = feature from previous pass
        3 => vec![(32, gs_vk.features[0].view)],
        4 => vec![(32, gs_vk.features[1].view)],
        5 => vec![(32, gs_vk.features[2].view)],

        // Pass 6: flow init — tex@32 = feature
        6 => vec![(32, gs_vk.features[0].view)],

        // Pass 7: coarse multi-scale OF — tex@32..37 = pyramid levels
        7 => vec![
            (32, gs_vk.pyramid[0].view),
            (33, gs_vk.pyramid[1].view),
            (34, gs_vk.pyramid[2].view),
            (35, gs_vk.pyramid[3].view),
            (36, gs_vk.pyramid[4].view),
            (37, gs_vk.pyramid[5].view),
        ],

        // Pass 8: OF refinement — tex@32 = prev feature, tex@33 = curr feature
        8 => vec![(32, gs_vk.features[0].view), (33, gs_vk.features[1].view)],

        // Pass 9-11: iterative OF refinement — tex@32 = feature A, tex@33 = feature B
        9 => vec![(32, gs_vk.features[0].view), (33, gs_vk.features[1].view)],
        10 => vec![(32, gs_vk.features[0].view), (33, gs_vk.features[1].view)],
        11 => vec![(32, gs_vk.features[0].view), (33, gs_vk.features[1].view)],

        // Pass 12: final full-res OF — tex@32 = feature A, tex@33 = feature B
        12 => vec![(32, gs_vk.features[3].view), (33, gs_vk.features[4].view)],

        // Pass 13: flow pyramid expand — tex@32 = flow, tex@33 = feature
        13 => vec![(32, gs_vk.flow_fwd.view), (33, gs_vk.flow_bwd.view)],

        // Pass 14: multi-scale aggregation — tex@32..37 = flow pyramid
        14 => vec![
            (32, gs_vk.pyramid[0].view),
            (33, gs_vk.pyramid[1].view),
            (34, gs_vk.pyramid[2].view),
            (35, gs_vk.pyramid[3].view),
            (36, gs_vk.pyramid[4].view),
            (37, gs_vk.pyramid[5].view),
        ],

        // Pass 15: flow merge — tex@32 = fwd, tex@33 = bwd
        15 => vec![(32, gs_vk.flow_fwd.view), (33, gs_vk.flow_bwd.view)],

        // Pass 16: flow expand — tex@32 = merged flow
        16 => vec![(32, gs_vk.temp_a.view)],

        // Pass 17: flow warp + blend — tex@32..36 = frames + flow + confidence
        17 => vec![
            (32, history_view),
            (33, swapchain_view),
            (34, gs_vk.flow_fwd.view),
            (35, gs_vk.flow_bwd.view),
            (36, gs_vk.temp_a.view),
        ],

        // Pass 18: frame synthesis — tex@32..36 = frames + flow + confidence
        18 => vec![
            (32, history_view),
            (33, swapchain_view),
            (34, gs_vk.flow_fwd.view),
            (35, gs_vk.flow_bwd.view),
            (36, gs_vk.temp_a.view),
        ],

        _ => vec![],
    }
}

/// Returns the output storage image views (b48..) for a given pass.
fn pass_output_bindings(pass: usize, gs_vk: &GsVkPipeline) -> Vec<(u32, vk::ImageView)> {
    match pass {
        // Pass 0: pyramid builder — img@48..54 = 7 pyramid levels
        0 => gs_vk
            .pyramid
            .iter()
            .enumerate()
            .map(|(i, img)| (48 + i as u32, img.view))
            .collect(),

        // Pass 1-5: feature extraction — img@48 = feature map
        1 => vec![(48, gs_vk.features[0].view)],
        2 => vec![(48, gs_vk.features[1].view)],
        3 => vec![(48, gs_vk.features[2].view)],
        4 => vec![(48, gs_vk.features[3].view)],
        5 => vec![(48, gs_vk.features[4].view)],

        // Pass 6: flow init — img@48,49 = flow init A,B
        6 => vec![(48, gs_vk.flow_fwd.view), (49, gs_vk.flow_bwd.view)],

        // Pass 7: coarse OF — img@48,49 = coarse fwd,bwd flow
        7 => vec![(48, gs_vk.flow_fwd.view), (49, gs_vk.flow_bwd.view)],

        // Pass 8-12: OF refinement — img@48,49 = refined fwd,bwd flow
        8 => vec![(48, gs_vk.flow_fwd.view), (49, gs_vk.flow_bwd.view)],
        9 => vec![(48, gs_vk.flow_fwd.view), (49, gs_vk.flow_bwd.view)],
        10 => vec![(48, gs_vk.flow_fwd.view), (49, gs_vk.flow_bwd.view)],
        11 => vec![(48, gs_vk.flow_fwd.view), (49, gs_vk.flow_bwd.view)],
        12 => vec![(48, gs_vk.flow_fwd.view), (49, gs_vk.flow_bwd.view)],

        // Pass 13: flow pyramid expand — img@48..53 = flow pyramid levels
        13 => vec![
            (48, gs_vk.pyramid[0].view),
            (49, gs_vk.pyramid[1].view),
            (50, gs_vk.pyramid[2].view),
            (51, gs_vk.pyramid[3].view),
            (52, gs_vk.pyramid[4].view),
            (53, gs_vk.pyramid[5].view),
        ],

        // Pass 14: multi-scale aggregation — img@48 = aggregated flow
        14 => vec![(48, gs_vk.temp_a.view)],

        // Pass 15: flow merge — img@48 = merged flow
        15 => vec![(48, gs_vk.temp_a.view)],

        // Pass 16: flow expand — img@48,49 = expanded flow A,B
        16 => vec![(48, gs_vk.flow_fwd.view), (49, gs_vk.flow_bwd.view)],

        // Pass 17: flow warp + blend — img@48,49,50 = warped A, B, blend
        17 => vec![
            (48, gs_vk.warp[0].view),
            (49, gs_vk.warp[1].view),
            (50, gs_vk.warp[2].view),
        ],

        // Pass 18: frame synthesis — img@48 = output interpolated frame
        18 => vec![(48, gs_vk.output.view)],

        _ => vec![],
    }
}

// ---------------------------------------------------------------------------
// Frame descriptor update
// ---------------------------------------------------------------------------

/// Update per-pass descriptor sets with the frame-specific image views.
/// This must be called each frame before recording dispatch commands,
/// because the swapchain image index changes between frames.
///
/// # Safety
/// The pipeline must be initialized. The history and swapchain views must
/// be valid for the lifetime of the descriptor sets.
pub unsafe fn update_frame_descriptors(
    device: vk::Device,
    dispatch: &crate::DeviceDispatch,
    gs_vk: &GsVkPipeline,
    history_view: vk::ImageView,
    swapchain_view: vk::ImageView,
) {
    let Some(update_descriptor_sets) = dispatch.update_descriptor_sets else {
        return;
    };

    let mut all_writes = Vec::new();
    let mut buffer_infos = Vec::new();
    let mut image_infos = Vec::new();

    for pass in 0..19usize {
        let ds = gs_vk.descriptor_sets[pass];

        // UBO binding (b0) — shared across all passes
        buffer_infos.push(vk::DescriptorBufferInfo {
            buffer: gs_vk.ubo_buffer,
            offset: 0,
            range: std::mem::size_of::<GsVkUbo>() as vk::DeviceSize,
        });
        let buf_idx = buffer_infos.len() - 1;
        all_writes.push(vk::WriteDescriptorSet {
            s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
            dst_set: ds,
            dst_binding: 0,
            dst_array_element: 0,
            descriptor_count: 1,
            descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
            p_buffer_info: &buffer_infos[buf_idx],
            ..Default::default()
        });

        // Sampler at b31
        image_infos.push(vk::DescriptorImageInfo {
            sampler: gs_vk.sampler,
            image_view: vk::ImageView::null(),
            image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        });
        let img_idx = image_infos.len() - 1;
        all_writes.push(vk::WriteDescriptorSet {
            s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
            dst_set: ds,
            dst_binding: 31,
            dst_array_element: 0,
            descriptor_count: 1,
            descriptor_type: vk::DescriptorType::SAMPLER,
            p_image_info: &image_infos[img_idx],
            ..Default::default()
        });

        // Input textures (b32..)
        for (binding, view) in pass_input_bindings(pass, history_view, swapchain_view, gs_vk) {
            image_infos.push(vk::DescriptorImageInfo {
                sampler: gs_vk.sampler,
                image_view: view,
                image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            });
            let img_idx = image_infos.len() - 1;
            all_writes.push(vk::WriteDescriptorSet {
                s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
                dst_set: ds,
                dst_binding: binding,
                dst_array_element: 0,
                descriptor_count: 1,
                descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                p_image_info: &image_infos[img_idx],
                ..Default::default()
            });
        }

        // Output storage images (b48..)
        for (binding, view) in pass_output_bindings(pass, gs_vk) {
            image_infos.push(vk::DescriptorImageInfo {
                sampler: vk::Sampler::null(),
                image_view: view,
                image_layout: vk::ImageLayout::GENERAL,
            });
            let img_idx = image_infos.len() - 1;
            all_writes.push(vk::WriteDescriptorSet {
                s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
                dst_set: ds,
                dst_binding: binding,
                dst_array_element: 0,
                descriptor_count: 1,
                descriptor_type: vk::DescriptorType::STORAGE_IMAGE,
                p_image_info: &image_infos[img_idx],
                ..Default::default()
            });
        }
    }

    update_descriptor_sets(
        device,
        all_writes.len() as u32,
        all_writes.as_ptr(),
        0,
        ptr::null(),
    );
}

pub unsafe fn record_gs_vk_dispatch(
    gs_vk: &GsVkPipeline,
    dispatch: &crate::DeviceDispatch,
    device: vk::Device,
    command_buffer: vk::CommandBuffer,
    frame_w: u32,
    frame_h: u32,
) {
    let Some(cmd_bind_pipeline) = dispatch.cmd_bind_pipeline else {
        return;
    };
    let Some(cmd_bind_descriptor_sets) = dispatch.cmd_bind_descriptor_sets else {
        return;
    };
    let Some(cmd_dispatch) = dispatch.cmd_dispatch else {
        return;
    };
    let Some(cmd_pipeline_barrier) = dispatch.cmd_pipeline_barrier else {
        return;
    };

    let compute_barrier = vk::MemoryBarrier {
        s_type: vk::StructureType::MEMORY_BARRIER,
        src_access_mask: vk::AccessFlags::SHADER_WRITE,
        dst_access_mask: vk::AccessFlags::SHADER_READ,
        ..Default::default()
    };

    for pass in 0..19usize {
        // Bind pipeline
        cmd_bind_pipeline(
            command_buffer,
            vk::PipelineBindPoint::COMPUTE,
            gs_vk.pipelines[pass],
        );

        // Bind descriptor set
        cmd_bind_descriptor_sets(
            command_buffer,
            vk::PipelineBindPoint::COMPUTE,
            gs_vk.pipeline_layout,
            0,
            1,
            &gs_vk.descriptor_sets[pass],
            0,
            ptr::null(),
        );

        // Dispatch
        let (groups_x, groups_y) = pass_dispatch_extent(pass, frame_w, frame_h);
        cmd_dispatch(command_buffer, groups_x, groups_y, 1);

        // Barrier between passes — ensure shader writes are visible to next pass
        if pass < 18 {
            cmd_pipeline_barrier(
                command_buffer,
                vk::PipelineStageFlags::COMPUTE_SHADER,
                vk::PipelineStageFlags::COMPUTE_SHADER,
                vk::DependencyFlags::empty(),
                1,
                &compute_barrier,
                0,
                ptr::null(),
                0,
                ptr::null(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ash::vk::Handle;

    #[test]
    fn pass_dispatch_extent_full_res_passes() {
        // Full resolution passes should use ceil(W/16) × ceil(H/16)
        let (gx, gy) = pass_dispatch_extent(0, 1920, 1080);
        assert_eq!(gx, 120); // ceil(1920/16)
        assert_eq!(gy, 68);  // ceil(1080/16)

        let (gx, gy) = pass_dispatch_extent(12, 1920, 1080);
        assert_eq!(gx, 120);
        assert_eq!(gy, 68);

        let (gx, gy) = pass_dispatch_extent(18, 1920, 1080);
        assert_eq!(gx, 120);
        assert_eq!(gy, 68);
    }

    #[test]
    fn pass_dispatch_extent_half_res_passes() {
        // Half-resolution passes (feature extraction, OF refinement)
        let (gx, gy) = pass_dispatch_extent(1, 1920, 1080);
        assert_eq!(gx, 60);  // ceil(960/16)
        assert_eq!(gy, 34);  // ceil(540/16)

        let (gx, gy) = pass_dispatch_extent(8, 1920, 1080);
        assert_eq!(gx, 60);
        assert_eq!(gy, 34);
    }

    #[test]
    fn pass_dispatch_extent_coarse_of() {
        // Coarse multi-scale OF at 1/8 resolution
        let (gx, gy) = pass_dispatch_extent(7, 1920, 1080);
        assert_eq!(gx, 15);  // ceil(240/16)
        assert_eq!(gy, 9);   // ceil(135/16)
    }

    #[test]
    fn pass_dispatch_extent_iterative_of() {
        // Iterative OF at increasing resolutions
        let (gx, gy) = pass_dispatch_extent(9, 1920, 1080);  // 1/4 res
        assert_eq!(gx, 30);  // ceil(480/16)
        assert_eq!(gy, 17);  // ceil(270/16)

        let (gx, gy) = pass_dispatch_extent(10, 1920, 1080); // 1/2 res
        assert_eq!(gx, 60);
        assert_eq!(gy, 34);

        let (gx, gy) = pass_dispatch_extent(11, 1920, 1080); // full res
        assert_eq!(gx, 120);
        assert_eq!(gy, 68);
    }

    #[test]
    fn pass_input_bindings_synthesis_uses_source_frames() {
        let gs_vk = GsVkPipeline::new();
        let history = vk::ImageView::from_raw(100);
        let swapchain = vk::ImageView::from_raw(200);

        // Pass 18 (frame synthesis) should bind both frames + flow + confidence
        let bindings = pass_input_bindings(18, history, swapchain, &gs_vk);
        assert_eq!(bindings.len(), 5);
        assert_eq!(bindings[0].0, 32); // b32 = prev frame
        assert_eq!(bindings[1].0, 33); // b33 = curr frame
        assert_eq!(bindings[2].0, 34); // b34 = fwd flow
        assert_eq!(bindings[3].0, 35); // b35 = bwd flow
        assert_eq!(bindings[4].0, 36); // b36 = confidence
    }

    #[test]
    fn pass_output_bindings_pyramid_produces_7_levels() {
        let gs_vk = GsVkPipeline::new();
        let bindings = pass_output_bindings(0, &gs_vk);
        assert_eq!(bindings.len(), 7);
        assert_eq!(bindings[0].0, 48);
        assert_eq!(bindings[6].0, 54);
    }
}
