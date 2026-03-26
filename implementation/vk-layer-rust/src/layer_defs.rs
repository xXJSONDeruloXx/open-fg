use ash::vk;
use std::ffi::{c_char, c_void};

pub const CURRENT_LOADER_LAYER_INTERFACE_VERSION: u32 = 2;

pub type PfnGetPhysicalDeviceProcAddr = unsafe extern "system" fn(
    instance: vk::Instance,
    p_name: *const c_char,
) -> vk::PFN_vkVoidFunction;

pub type PfnVkLayerCreateDevice = *const c_void;
pub type PfnVkLayerDestroyDevice = *const c_void;

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VkNegotiateLayerStructType {
    Uninitialized = 0,
    Interface = 1,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VkNegotiateLayerInterface {
    pub s_type: VkNegotiateLayerStructType,
    pub p_next: *mut c_void,
    pub loader_layer_interface_version: u32,
    pub pfn_get_instance_proc_addr: vk::PFN_vkGetInstanceProcAddr,
    pub pfn_get_device_proc_addr: vk::PFN_vkGetDeviceProcAddr,
    pub pfn_get_physical_device_proc_addr: Option<PfnGetPhysicalDeviceProcAddr>,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VkLayerFunction {
    LinkInfo = 0,
    LoaderDataCallback = 1,
    LoaderLayerCreateDeviceCallback = 2,
    LoaderFeatures = 3,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VkLayerInstanceLink {
    pub p_next: *mut VkLayerInstanceLink,
    pub pfn_next_get_instance_proc_addr: vk::PFN_vkGetInstanceProcAddr,
    pub pfn_next_get_physical_device_proc_addr: Option<PfnGetPhysicalDeviceProcAddr>,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VkLayerDeviceInfo {
    pub device_info: *mut c_void,
    pub pfn_next_get_instance_proc_addr: vk::PFN_vkGetInstanceProcAddr,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VkLayerDeviceCallbacks {
    pub pfn_layer_create_device: PfnVkLayerCreateDevice,
    pub pfn_layer_destroy_device: PfnVkLayerDestroyDevice,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union VkLayerInstanceCreateInfoUnion {
    pub p_layer_info: *mut VkLayerInstanceLink,
    pub pfn_set_instance_loader_data: *const c_void,
    pub layer_device: VkLayerDeviceCallbacks,
    pub loader_features: vk::Flags,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VkLayerInstanceCreateInfo {
    pub s_type: vk::StructureType,
    pub p_next: *const c_void,
    pub function: VkLayerFunction,
    pub u: VkLayerInstanceCreateInfoUnion,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VkLayerDeviceLink {
    pub p_next: *mut VkLayerDeviceLink,
    pub pfn_next_get_instance_proc_addr: vk::PFN_vkGetInstanceProcAddr,
    pub pfn_next_get_device_proc_addr: vk::PFN_vkGetDeviceProcAddr,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union VkLayerDeviceCreateInfoUnion {
    pub p_layer_info: *mut VkLayerDeviceLink,
    pub pfn_set_device_loader_data: *const c_void,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VkLayerDeviceCreateInfo {
    pub s_type: vk::StructureType,
    pub p_next: *const c_void,
    pub function: VkLayerFunction,
    pub u: VkLayerDeviceCreateInfoUnion,
}
