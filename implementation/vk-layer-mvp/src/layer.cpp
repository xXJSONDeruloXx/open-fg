#include <vulkan/vk_layer.h>
#include <vulkan/vulkan.h>

#include <algorithm>
#include <array>
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <mutex>
#include <optional>
#include <string>
#include <string_view>
#include <unordered_map>
#include <utility>
#include <vector>

namespace ppfg {

constexpr const char* kLayerName = "VK_LAYER_PPFG_mvp";

#if defined(__GNUC__) || defined(__clang__)
#define PPFG_EXPORT __attribute__((visibility("default")))
#else
#define PPFG_EXPORT
#endif

struct Logger {
    std::once_flag init_once;
    std::mutex mutex;
    FILE* sink = stderr;
    bool owns_sink = false;

    ~Logger() {
        if (owns_sink && sink) {
            std::fclose(sink);
        }
    }

    static Logger& instance() {
        static Logger logger;
        return logger;
    }

    void init() {
        std::call_once(init_once, [&] {
            const char* path = std::getenv("PPFG_LAYER_LOG_FILE");
            if (path && *path) {
                if (FILE* file = std::fopen(path, "a")) {
                    sink = file;
                    owns_sink = true;
                }
            }
        });
    }

    void log(const char* level, const std::string& message) {
        init();
        const auto now = std::chrono::system_clock::now();
        const auto epoch_ms = std::chrono::duration_cast<std::chrono::milliseconds>(
            now.time_since_epoch()).count();

        std::lock_guard<std::mutex> lock(mutex);
        std::fprintf(sink, "[ppfg][%s][%lld] %s\n", level,
            static_cast<long long>(epoch_ms), message.c_str());
        std::fflush(sink);
    }
};

void log_info(const std::string& message) {
    Logger::instance().log("info", message);
}

void log_warn(const std::string& message) {
    Logger::instance().log("warn", message);
}

void log_error(const std::string& message) {
    Logger::instance().log("error", message);
}

template <typename Dispatchable>
void* get_key(Dispatchable handle) {
    if (!handle) {
        return nullptr;
    }
    return *reinterpret_cast<void**>(handle);
}

enum class Mode {
    PassThrough,
    ClearTest,
    CopyTest,
    HistoryCopyTest,
};

Mode current_mode() {
    const char* mode = std::getenv("PPFG_LAYER_MODE");
    if (!mode || !*mode) {
        return Mode::PassThrough;
    }

    if (std::strcmp(mode, "clear") == 0 || std::strcmp(mode, "clear-test") == 0) {
        return Mode::ClearTest;
    }
    if (std::strcmp(mode, "copy") == 0 || std::strcmp(mode, "copy-test") == 0 || std::strcmp(mode, "duplicate") == 0) {
        return Mode::CopyTest;
    }
    if (std::strcmp(mode, "history") == 0 || std::strcmp(mode, "history-copy") == 0
            || std::strcmp(mode, "copy-prev") == 0 || std::strcmp(mode, "history-copy-test") == 0) {
        return Mode::HistoryCopyTest;
    }

    return Mode::PassThrough;
}

const char* mode_name(Mode mode) {
    switch (mode) {
        case Mode::PassThrough:
            return "passthrough";
        case Mode::ClearTest:
            return "clear-test";
        case Mode::CopyTest:
            return "copy-test";
        case Mode::HistoryCopyTest:
            return "history-copy-test";
    }
    return "unknown";
}

std::string present_mode_name(VkPresentModeKHR mode) {
    switch (mode) {
        case VK_PRESENT_MODE_IMMEDIATE_KHR:
            return "IMMEDIATE";
        case VK_PRESENT_MODE_MAILBOX_KHR:
            return "MAILBOX";
        case VK_PRESENT_MODE_FIFO_KHR:
            return "FIFO";
        case VK_PRESENT_MODE_FIFO_RELAXED_KHR:
            return "FIFO_RELAXED";
#ifdef VK_PRESENT_MODE_SHARED_DEMAND_REFRESH_KHR
        case VK_PRESENT_MODE_SHARED_DEMAND_REFRESH_KHR:
            return "SHARED_DEMAND_REFRESH";
#endif
#ifdef VK_PRESENT_MODE_SHARED_CONTINUOUS_REFRESH_KHR
        case VK_PRESENT_MODE_SHARED_CONTINUOUS_REFRESH_KHR:
            return "SHARED_CONTINUOUS_REFRESH";
#endif
        default:
            break;
    }

    return std::to_string(static_cast<int>(mode));
}

std::string format_extent(VkExtent2D extent) {
    return std::to_string(extent.width) + "x" + std::to_string(extent.height);
}

std::string format_hex(uint64_t value) {
    char buffer[32]{};
    std::snprintf(buffer, sizeof(buffer), "0x%llx", static_cast<unsigned long long>(value));
    return std::string(buffer);
}

uint64_t queue_id(VkQueue queue) {
    return static_cast<uint64_t>(reinterpret_cast<uintptr_t>(queue));
}

std::string usage_flags(VkImageUsageFlags flags) {
    struct NamedFlag {
        VkImageUsageFlags bit;
        const char* name;
    };

    constexpr std::array<NamedFlag, 8> names{{
        {VK_IMAGE_USAGE_TRANSFER_SRC_BIT, "TRANSFER_SRC"},
        {VK_IMAGE_USAGE_TRANSFER_DST_BIT, "TRANSFER_DST"},
        {VK_IMAGE_USAGE_SAMPLED_BIT, "SAMPLED"},
        {VK_IMAGE_USAGE_STORAGE_BIT, "STORAGE"},
        {VK_IMAGE_USAGE_COLOR_ATTACHMENT_BIT, "COLOR_ATTACHMENT"},
        {VK_IMAGE_USAGE_DEPTH_STENCIL_ATTACHMENT_BIT, "DEPTH_STENCIL_ATTACHMENT"},
        {VK_IMAGE_USAGE_TRANSIENT_ATTACHMENT_BIT, "TRANSIENT_ATTACHMENT"},
        {VK_IMAGE_USAGE_INPUT_ATTACHMENT_BIT, "INPUT_ATTACHMENT"},
    }};

    std::string result;
    for (const auto& named : names) {
        if ((flags & named.bit) == 0) {
            continue;
        }
        if (!result.empty()) {
            result += '|';
        }
        result += named.name;
    }
    if (result.empty()) {
        result = "0";
    }
    result += " (" + format_hex(flags) + ')';
    return result;
}

struct InstanceDispatch {
    PFN_vkGetInstanceProcAddr GetInstanceProcAddr = nullptr;
    PFN_vkDestroyInstance DestroyInstance = nullptr;
    PFN_vkCreateDevice CreateDevice = nullptr;
    PFN_vkEnumerateDeviceExtensionProperties EnumerateDeviceExtensionProperties = nullptr;
    PFN_vkGetPhysicalDeviceProperties GetPhysicalDeviceProperties = nullptr;
    PFN_vkGetPhysicalDeviceQueueFamilyProperties GetPhysicalDeviceQueueFamilyProperties = nullptr;
    PFN_vkGetPhysicalDeviceMemoryProperties GetPhysicalDeviceMemoryProperties = nullptr;
    PFN_vkGetPhysicalDeviceSurfaceCapabilitiesKHR GetPhysicalDeviceSurfaceCapabilitiesKHR = nullptr;
};

struct DeviceDispatch {
    PFN_vkGetDeviceProcAddr GetDeviceProcAddr = nullptr;
    PFN_vkDestroyDevice DestroyDevice = nullptr;
    PFN_vkGetDeviceQueue GetDeviceQueue = nullptr;
    PFN_vkGetDeviceQueue2 GetDeviceQueue2 = nullptr;
    PFN_vkQueuePresentKHR QueuePresentKHR = nullptr;
    PFN_vkCreateSwapchainKHR CreateSwapchainKHR = nullptr;
    PFN_vkDestroySwapchainKHR DestroySwapchainKHR = nullptr;
    PFN_vkGetSwapchainImagesKHR GetSwapchainImagesKHR = nullptr;
    PFN_vkAcquireNextImageKHR AcquireNextImageKHR = nullptr;
    PFN_vkCreateCommandPool CreateCommandPool = nullptr;
    PFN_vkDestroyCommandPool DestroyCommandPool = nullptr;
    PFN_vkResetCommandPool ResetCommandPool = nullptr;
    PFN_vkAllocateCommandBuffers AllocateCommandBuffers = nullptr;
    PFN_vkFreeCommandBuffers FreeCommandBuffers = nullptr;
    PFN_vkBeginCommandBuffer BeginCommandBuffer = nullptr;
    PFN_vkEndCommandBuffer EndCommandBuffer = nullptr;
    PFN_vkQueueSubmit QueueSubmit = nullptr;
    PFN_vkCreateFence CreateFence = nullptr;
    PFN_vkDestroyFence DestroyFence = nullptr;
    PFN_vkWaitForFences WaitForFences = nullptr;
    PFN_vkResetFences ResetFences = nullptr;
    PFN_vkCreateSemaphore CreateSemaphore = nullptr;
    PFN_vkDestroySemaphore DestroySemaphore = nullptr;
    PFN_vkQueueWaitIdle QueueWaitIdle = nullptr;
    PFN_vkCreateImage CreateImage = nullptr;
    PFN_vkDestroyImage DestroyImage = nullptr;
    PFN_vkGetImageMemoryRequirements GetImageMemoryRequirements = nullptr;
    PFN_vkAllocateMemory AllocateMemory = nullptr;
    PFN_vkFreeMemory FreeMemory = nullptr;
    PFN_vkBindImageMemory BindImageMemory = nullptr;
    PFN_vkCmdPipelineBarrier CmdPipelineBarrier = nullptr;
    PFN_vkCmdClearColorImage CmdClearColorImage = nullptr;
    PFN_vkCmdCopyImage CmdCopyImage = nullptr;
    PFN_vkDeviceWaitIdle DeviceWaitIdle = nullptr;
};

struct QueueInfo {
    VkDevice device = VK_NULL_HANDLE;
    uint32_t family_index = 0;
    uint32_t queue_index = 0;
    bool supports_graphics = false;
    bool supports_transfer = false;
};

struct InjectResources {
    bool initialized = false;
    uint32_t family_index = 0;
    VkCommandPool command_pool = VK_NULL_HANDLE;
    VkCommandBuffer command_buffer = VK_NULL_HANDLE;
    VkSemaphore acquire_semaphore = VK_NULL_HANDLE;
    VkSemaphore ready_original_semaphore = VK_NULL_HANDLE;
    VkSemaphore ready_generated_semaphore = VK_NULL_HANDLE;
    VkFence submit_fence = VK_NULL_HANDLE;
};

struct SwapchainState {
    VkDevice device = VK_NULL_HANDLE;
    VkPhysicalDevice physical_device = VK_NULL_HANDLE;
    VkSurfaceKHR surface = VK_NULL_HANDLE;
    VkSwapchainKHR handle = VK_NULL_HANDLE;
    VkFormat format = VK_FORMAT_UNDEFINED;
    VkExtent2D extent{};
    VkPresentModeKHR present_mode = VK_PRESENT_MODE_FIFO_KHR;
    VkImageUsageFlags original_usage = 0;
    VkImageUsageFlags modified_usage = 0;
    uint32_t original_min_image_count = 0;
    uint32_t modified_min_image_count = 0;
    std::vector<VkImage> images;
    VkImage history_image = VK_NULL_HANDLE;
    VkDeviceMemory history_memory = VK_NULL_HANDLE;
    bool history_valid = false;
    uint64_t present_count = 0;
    uint64_t generated_present_count = 0;
    bool injection_attempted = false;
    bool injection_works = false;
    InjectResources inject;
};

struct DeviceInfo {
    VkInstance instance = VK_NULL_HANDLE;
    VkPhysicalDevice physical_device = VK_NULL_HANDLE;
    VkDevice device = VK_NULL_HANDLE;
    InstanceDispatch instance_dispatch{};
    DeviceDispatch dispatch{};
};

std::mutex g_mutex;
std::unordered_map<void*, InstanceDispatch> g_instance_dispatch;
std::unordered_map<void*, VkInstance> g_instance_map;
std::unordered_map<void*, DeviceInfo> g_device_map;
std::unordered_map<uint64_t, QueueInfo> g_queue_map;
std::unordered_map<VkSwapchainKHR, SwapchainState> g_swapchains;

void fill_instance_dispatch(VkInstance instance, PFN_vkGetInstanceProcAddr gipa, InstanceDispatch& dispatch) {
    dispatch.GetInstanceProcAddr = gipa;
    dispatch.DestroyInstance = reinterpret_cast<PFN_vkDestroyInstance>(gipa(instance, "vkDestroyInstance"));
    dispatch.CreateDevice = reinterpret_cast<PFN_vkCreateDevice>(gipa(instance, "vkCreateDevice"));
    dispatch.EnumerateDeviceExtensionProperties = reinterpret_cast<PFN_vkEnumerateDeviceExtensionProperties>(
        gipa(instance, "vkEnumerateDeviceExtensionProperties"));
    dispatch.GetPhysicalDeviceProperties = reinterpret_cast<PFN_vkGetPhysicalDeviceProperties>(
        gipa(instance, "vkGetPhysicalDeviceProperties"));
    dispatch.GetPhysicalDeviceQueueFamilyProperties = reinterpret_cast<PFN_vkGetPhysicalDeviceQueueFamilyProperties>(
        gipa(instance, "vkGetPhysicalDeviceQueueFamilyProperties"));
    dispatch.GetPhysicalDeviceMemoryProperties = reinterpret_cast<PFN_vkGetPhysicalDeviceMemoryProperties>(
        gipa(instance, "vkGetPhysicalDeviceMemoryProperties"));
    dispatch.GetPhysicalDeviceSurfaceCapabilitiesKHR = reinterpret_cast<PFN_vkGetPhysicalDeviceSurfaceCapabilitiesKHR>(
        gipa(instance, "vkGetPhysicalDeviceSurfaceCapabilitiesKHR"));
}

void fill_device_dispatch(VkDevice device, PFN_vkGetDeviceProcAddr gdpa, DeviceDispatch& dispatch) {
    dispatch.GetDeviceProcAddr = gdpa;
    dispatch.DestroyDevice = reinterpret_cast<PFN_vkDestroyDevice>(gdpa(device, "vkDestroyDevice"));
    dispatch.GetDeviceQueue = reinterpret_cast<PFN_vkGetDeviceQueue>(gdpa(device, "vkGetDeviceQueue"));
    dispatch.GetDeviceQueue2 = reinterpret_cast<PFN_vkGetDeviceQueue2>(gdpa(device, "vkGetDeviceQueue2"));
    dispatch.QueuePresentKHR = reinterpret_cast<PFN_vkQueuePresentKHR>(gdpa(device, "vkQueuePresentKHR"));
    dispatch.CreateSwapchainKHR = reinterpret_cast<PFN_vkCreateSwapchainKHR>(gdpa(device, "vkCreateSwapchainKHR"));
    dispatch.DestroySwapchainKHR = reinterpret_cast<PFN_vkDestroySwapchainKHR>(gdpa(device, "vkDestroySwapchainKHR"));
    dispatch.GetSwapchainImagesKHR = reinterpret_cast<PFN_vkGetSwapchainImagesKHR>(gdpa(device, "vkGetSwapchainImagesKHR"));
    dispatch.AcquireNextImageKHR = reinterpret_cast<PFN_vkAcquireNextImageKHR>(gdpa(device, "vkAcquireNextImageKHR"));
    dispatch.CreateCommandPool = reinterpret_cast<PFN_vkCreateCommandPool>(gdpa(device, "vkCreateCommandPool"));
    dispatch.DestroyCommandPool = reinterpret_cast<PFN_vkDestroyCommandPool>(gdpa(device, "vkDestroyCommandPool"));
    dispatch.ResetCommandPool = reinterpret_cast<PFN_vkResetCommandPool>(gdpa(device, "vkResetCommandPool"));
    dispatch.AllocateCommandBuffers = reinterpret_cast<PFN_vkAllocateCommandBuffers>(gdpa(device, "vkAllocateCommandBuffers"));
    dispatch.FreeCommandBuffers = reinterpret_cast<PFN_vkFreeCommandBuffers>(gdpa(device, "vkFreeCommandBuffers"));
    dispatch.BeginCommandBuffer = reinterpret_cast<PFN_vkBeginCommandBuffer>(gdpa(device, "vkBeginCommandBuffer"));
    dispatch.EndCommandBuffer = reinterpret_cast<PFN_vkEndCommandBuffer>(gdpa(device, "vkEndCommandBuffer"));
    dispatch.QueueSubmit = reinterpret_cast<PFN_vkQueueSubmit>(gdpa(device, "vkQueueSubmit"));
    dispatch.CreateFence = reinterpret_cast<PFN_vkCreateFence>(gdpa(device, "vkCreateFence"));
    dispatch.DestroyFence = reinterpret_cast<PFN_vkDestroyFence>(gdpa(device, "vkDestroyFence"));
    dispatch.WaitForFences = reinterpret_cast<PFN_vkWaitForFences>(gdpa(device, "vkWaitForFences"));
    dispatch.ResetFences = reinterpret_cast<PFN_vkResetFences>(gdpa(device, "vkResetFences"));
    dispatch.CreateSemaphore = reinterpret_cast<PFN_vkCreateSemaphore>(gdpa(device, "vkCreateSemaphore"));
    dispatch.DestroySemaphore = reinterpret_cast<PFN_vkDestroySemaphore>(gdpa(device, "vkDestroySemaphore"));
    dispatch.QueueWaitIdle = reinterpret_cast<PFN_vkQueueWaitIdle>(gdpa(device, "vkQueueWaitIdle"));
    dispatch.CreateImage = reinterpret_cast<PFN_vkCreateImage>(gdpa(device, "vkCreateImage"));
    dispatch.DestroyImage = reinterpret_cast<PFN_vkDestroyImage>(gdpa(device, "vkDestroyImage"));
    dispatch.GetImageMemoryRequirements = reinterpret_cast<PFN_vkGetImageMemoryRequirements>(gdpa(device, "vkGetImageMemoryRequirements"));
    dispatch.AllocateMemory = reinterpret_cast<PFN_vkAllocateMemory>(gdpa(device, "vkAllocateMemory"));
    dispatch.FreeMemory = reinterpret_cast<PFN_vkFreeMemory>(gdpa(device, "vkFreeMemory"));
    dispatch.BindImageMemory = reinterpret_cast<PFN_vkBindImageMemory>(gdpa(device, "vkBindImageMemory"));
    dispatch.CmdPipelineBarrier = reinterpret_cast<PFN_vkCmdPipelineBarrier>(gdpa(device, "vkCmdPipelineBarrier"));
    dispatch.CmdClearColorImage = reinterpret_cast<PFN_vkCmdClearColorImage>(gdpa(device, "vkCmdClearColorImage"));
    dispatch.CmdCopyImage = reinterpret_cast<PFN_vkCmdCopyImage>(gdpa(device, "vkCmdCopyImage"));
    dispatch.DeviceWaitIdle = reinterpret_cast<PFN_vkDeviceWaitIdle>(gdpa(device, "vkDeviceWaitIdle"));
}

void remember_queue(VkQueue queue, VkDevice device, uint32_t family_index, uint32_t queue_index,
        bool supports_graphics, bool supports_transfer) {
    if (!queue) {
        return;
    }

    QueueInfo info{};
    info.device = device;
    info.family_index = family_index;
    info.queue_index = queue_index;
    info.supports_graphics = supports_graphics;
    info.supports_transfer = supports_transfer;

    std::lock_guard<std::mutex> lock(g_mutex);
    g_queue_map[queue_id(queue)] = info;
}

bool wait_and_reset_fence(const DeviceDispatch& dispatch, VkDevice device, VkFence fence, const char* label) {
    if (!fence) {
        return true;
    }

    const VkResult wait_result = dispatch.WaitForFences(device, 1, &fence, VK_TRUE, 5'000'000'000ULL);
    if (wait_result != VK_SUCCESS) {
        log_warn(std::string("WaitForFences failed for ") + label + ": " + std::to_string(wait_result));
        return false;
    }

    const VkResult reset_result = dispatch.ResetFences(device, 1, &fence);
    if (reset_result != VK_SUCCESS) {
        log_warn(std::string("ResetFences failed for ") + label + ": " + std::to_string(reset_result));
        return false;
    }

    return true;
}

void destroy_inject_resources(const DeviceDispatch& dispatch, VkDevice device, InjectResources& inject) {
    if (!inject.initialized) {
        return;
    }

    if (dispatch.DeviceWaitIdle) {
        dispatch.DeviceWaitIdle(device);
    }

    if (inject.acquire_semaphore) {
        dispatch.DestroySemaphore(device, inject.acquire_semaphore, nullptr);
        inject.acquire_semaphore = VK_NULL_HANDLE;
    }
    if (inject.ready_original_semaphore) {
        dispatch.DestroySemaphore(device, inject.ready_original_semaphore, nullptr);
        inject.ready_original_semaphore = VK_NULL_HANDLE;
    }
    if (inject.ready_generated_semaphore) {
        dispatch.DestroySemaphore(device, inject.ready_generated_semaphore, nullptr);
        inject.ready_generated_semaphore = VK_NULL_HANDLE;
    }
    if (inject.submit_fence) {
        dispatch.DestroyFence(device, inject.submit_fence, nullptr);
        inject.submit_fence = VK_NULL_HANDLE;
    }
    if (inject.command_pool) {
        if (inject.command_buffer) {
            dispatch.FreeCommandBuffers(device, inject.command_pool, 1, &inject.command_buffer);
            inject.command_buffer = VK_NULL_HANDLE;
        }
        dispatch.DestroyCommandPool(device, inject.command_pool, nullptr);
        inject.command_pool = VK_NULL_HANDLE;
    }
    inject.initialized = false;
}

std::optional<uint32_t> find_memory_type_index(const DeviceInfo& device_info, uint32_t memory_type_bits, VkMemoryPropertyFlags required_flags) {
    if (!device_info.instance_dispatch.GetPhysicalDeviceMemoryProperties || device_info.physical_device == VK_NULL_HANDLE) {
        return std::nullopt;
    }

    VkPhysicalDeviceMemoryProperties memory_properties{};
    device_info.instance_dispatch.GetPhysicalDeviceMemoryProperties(device_info.physical_device, &memory_properties);
    for (uint32_t i = 0; i < memory_properties.memoryTypeCount; ++i) {
        if ((memory_type_bits & (1u << i)) == 0) {
            continue;
        }
        if ((memory_properties.memoryTypes[i].propertyFlags & required_flags) == required_flags) {
            return i;
        }
    }
    return std::nullopt;
}

bool ensure_history_image(SwapchainState& swapchain, const DeviceInfo& device_info) {
    if (swapchain.history_image != VK_NULL_HANDLE) {
        return true;
    }
    if (!device_info.dispatch.CreateImage || !device_info.dispatch.GetImageMemoryRequirements
            || !device_info.dispatch.AllocateMemory || !device_info.dispatch.BindImageMemory) {
        log_warn("history image creation functions unavailable");
        return false;
    }

    VkImageCreateInfo image_info{};
    image_info.sType = VK_STRUCTURE_TYPE_IMAGE_CREATE_INFO;
    image_info.imageType = VK_IMAGE_TYPE_2D;
    image_info.format = swapchain.format;
    image_info.extent.width = swapchain.extent.width;
    image_info.extent.height = swapchain.extent.height;
    image_info.extent.depth = 1;
    image_info.mipLevels = 1;
    image_info.arrayLayers = 1;
    image_info.samples = VK_SAMPLE_COUNT_1_BIT;
    image_info.tiling = VK_IMAGE_TILING_OPTIMAL;
    image_info.usage = VK_IMAGE_USAGE_TRANSFER_SRC_BIT | VK_IMAGE_USAGE_TRANSFER_DST_BIT;
    image_info.sharingMode = VK_SHARING_MODE_EXCLUSIVE;
    image_info.initialLayout = VK_IMAGE_LAYOUT_UNDEFINED;

    if (device_info.dispatch.CreateImage(device_info.device, &image_info, nullptr, &swapchain.history_image) != VK_SUCCESS) {
        log_warn("CreateImage failed for history image");
        swapchain.history_image = VK_NULL_HANDLE;
        return false;
    }

    VkMemoryRequirements memory_requirements{};
    device_info.dispatch.GetImageMemoryRequirements(device_info.device, swapchain.history_image, &memory_requirements);
    const auto memory_type_index = find_memory_type_index(device_info, memory_requirements.memoryTypeBits, VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT);
    if (!memory_type_index.has_value()) {
        log_warn("failed to find device-local memory type for history image");
        device_info.dispatch.DestroyImage(device_info.device, swapchain.history_image, nullptr);
        swapchain.history_image = VK_NULL_HANDLE;
        return false;
    }

    VkMemoryAllocateInfo alloc_info{};
    alloc_info.sType = VK_STRUCTURE_TYPE_MEMORY_ALLOCATE_INFO;
    alloc_info.allocationSize = memory_requirements.size;
    alloc_info.memoryTypeIndex = *memory_type_index;
    if (device_info.dispatch.AllocateMemory(device_info.device, &alloc_info, nullptr, &swapchain.history_memory) != VK_SUCCESS) {
        log_warn("AllocateMemory failed for history image");
        device_info.dispatch.DestroyImage(device_info.device, swapchain.history_image, nullptr);
        swapchain.history_image = VK_NULL_HANDLE;
        swapchain.history_memory = VK_NULL_HANDLE;
        return false;
    }

    if (device_info.dispatch.BindImageMemory(device_info.device, swapchain.history_image, swapchain.history_memory, 0) != VK_SUCCESS) {
        log_warn("BindImageMemory failed for history image");
        device_info.dispatch.FreeMemory(device_info.device, swapchain.history_memory, nullptr);
        device_info.dispatch.DestroyImage(device_info.device, swapchain.history_image, nullptr);
        swapchain.history_memory = VK_NULL_HANDLE;
        swapchain.history_image = VK_NULL_HANDLE;
        return false;
    }

    swapchain.history_valid = false;
    log_info("created history image for swapchain");
    return true;
}

void destroy_swapchain_resources(const DeviceInfo& device_info, SwapchainState& swapchain) {
    if (device_info.dispatch.DeviceWaitIdle) {
        device_info.dispatch.DeviceWaitIdle(device_info.device);
    }
    destroy_inject_resources(device_info.dispatch, device_info.device, swapchain.inject);
    swapchain.history_valid = false;
    if (swapchain.history_image != VK_NULL_HANDLE) {
        device_info.dispatch.DestroyImage(device_info.device, swapchain.history_image, nullptr);
        swapchain.history_image = VK_NULL_HANDLE;
    }
    if (swapchain.history_memory != VK_NULL_HANDLE) {
        device_info.dispatch.FreeMemory(device_info.device, swapchain.history_memory, nullptr);
        swapchain.history_memory = VK_NULL_HANDLE;
    }
}

bool init_inject_resources(SwapchainState& swapchain, const DeviceInfo& device_info, const QueueInfo& queue_info) {
    if (swapchain.inject.initialized) {
        if (swapchain.inject.family_index == queue_info.family_index) {
            return true;
        }
        destroy_inject_resources(device_info.dispatch, device_info.device, swapchain.inject);
    }

    if (!queue_info.supports_graphics && !queue_info.supports_transfer) {
        log_warn("present queue family has neither graphics nor transfer support; skipping injection");
        return false;
    }

    VkCommandPoolCreateInfo pool_info{};
    pool_info.sType = VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO;
    pool_info.flags = VK_COMMAND_POOL_CREATE_RESET_COMMAND_BUFFER_BIT;
    pool_info.queueFamilyIndex = queue_info.family_index;

    if (device_info.dispatch.CreateCommandPool(device_info.device, &pool_info, nullptr, &swapchain.inject.command_pool) != VK_SUCCESS) {
        log_warn("CreateCommandPool failed for injection resources");
        return false;
    }

    VkCommandBufferAllocateInfo alloc_info{};
    alloc_info.sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO;
    alloc_info.commandPool = swapchain.inject.command_pool;
    alloc_info.level = VK_COMMAND_BUFFER_LEVEL_PRIMARY;
    alloc_info.commandBufferCount = 1;

    if (device_info.dispatch.AllocateCommandBuffers(device_info.device, &alloc_info, &swapchain.inject.command_buffer) != VK_SUCCESS) {
        log_warn("AllocateCommandBuffers failed for injection resources");
        destroy_inject_resources(device_info.dispatch, device_info.device, swapchain.inject);
        return false;
    }

    VkSemaphoreCreateInfo semaphore_info{};
    semaphore_info.sType = VK_STRUCTURE_TYPE_SEMAPHORE_CREATE_INFO;
    if (device_info.dispatch.CreateSemaphore(device_info.device, &semaphore_info, nullptr, &swapchain.inject.acquire_semaphore) != VK_SUCCESS) {
        log_warn("CreateSemaphore failed for acquire semaphore");
        destroy_inject_resources(device_info.dispatch, device_info.device, swapchain.inject);
        return false;
    }
    if (device_info.dispatch.CreateSemaphore(device_info.device, &semaphore_info, nullptr, &swapchain.inject.ready_original_semaphore) != VK_SUCCESS) {
        log_warn("CreateSemaphore failed for original-ready semaphore");
        destroy_inject_resources(device_info.dispatch, device_info.device, swapchain.inject);
        return false;
    }
    if (device_info.dispatch.CreateSemaphore(device_info.device, &semaphore_info, nullptr, &swapchain.inject.ready_generated_semaphore) != VK_SUCCESS) {
        log_warn("CreateSemaphore failed for generated-ready semaphore");
        destroy_inject_resources(device_info.dispatch, device_info.device, swapchain.inject);
        return false;
    }

    VkFenceCreateInfo submit_fence_info{};
    submit_fence_info.sType = VK_STRUCTURE_TYPE_FENCE_CREATE_INFO;
    submit_fence_info.flags = VK_FENCE_CREATE_SIGNALED_BIT;
    if (device_info.dispatch.CreateFence(device_info.device, &submit_fence_info, nullptr, &swapchain.inject.submit_fence) != VK_SUCCESS) {
        log_warn("CreateFence failed for submit fence");
        destroy_inject_resources(device_info.dispatch, device_info.device, swapchain.inject);
        return false;
    }

    swapchain.inject.initialized = true;
    swapchain.inject.family_index = queue_info.family_index;
    log_info("initialized injection resources for queue family " + std::to_string(queue_info.family_index));
    return true;
}

void refresh_swapchain_images(SwapchainState& state, const DeviceDispatch& dispatch) {
    uint32_t image_count = 0;
    VkResult result = dispatch.GetSwapchainImagesKHR(state.device, state.handle, &image_count, nullptr);
    if (result != VK_SUCCESS || image_count == 0) {
        log_warn("GetSwapchainImagesKHR(count) failed: " + std::to_string(result));
        return;
    }

    state.images.resize(image_count);
    result = dispatch.GetSwapchainImagesKHR(state.device, state.handle, &image_count, state.images.data());
    if (result != VK_SUCCESS) {
        log_warn("GetSwapchainImagesKHR(images) failed: " + std::to_string(result));
        state.images.clear();
        return;
    }
    state.images.resize(image_count);
}

VkImageMemoryBarrier image_barrier(VkImage image, VkAccessFlags src_access, VkAccessFlags dst_access,
        VkImageLayout old_layout, VkImageLayout new_layout) {
    VkImageMemoryBarrier barrier{};
    barrier.sType = VK_STRUCTURE_TYPE_IMAGE_MEMORY_BARRIER;
    barrier.srcAccessMask = src_access;
    barrier.dstAccessMask = dst_access;
    barrier.oldLayout = old_layout;
    barrier.newLayout = new_layout;
    barrier.srcQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED;
    barrier.dstQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED;
    barrier.image = image;
    barrier.subresourceRange.aspectMask = VK_IMAGE_ASPECT_COLOR_BIT;
    barrier.subresourceRange.baseMipLevel = 0;
    barrier.subresourceRange.levelCount = 1;
    barrier.subresourceRange.baseArrayLayer = 0;
    barrier.subresourceRange.layerCount = 1;
    return barrier;
}

bool try_present_copy_frame(SwapchainState& state, const DeviceInfo& device_info, const QueueInfo& queue_info, VkQueue queue,
        const VkPresentInfoKHR* present_info) {
    if (!init_inject_resources(state, device_info, queue_info)) {
        return false;
    }
    if (!present_info || present_info->swapchainCount != 1 || !device_info.dispatch.CmdCopyImage) {
        return false;
    }

    const VkResult prior_submit_wait = device_info.dispatch.WaitForFences(
        device_info.device,
        1,
        &state.inject.submit_fence,
        VK_TRUE,
        5'000'000'000ULL);
    if (prior_submit_wait != VK_SUCCESS) {
        log_warn("WaitForFences failed for submit fence: " + std::to_string(prior_submit_wait));
        return false;
    }

    uint32_t generated_image_index = 0;
    const VkResult acquire_result = device_info.dispatch.AcquireNextImageKHR(
        device_info.device,
        state.handle,
        20'000'000ULL,
        state.inject.acquire_semaphore,
        VK_NULL_HANDLE,
        &generated_image_index);
    if (acquire_result == VK_TIMEOUT || acquire_result == VK_NOT_READY) {
        log_warn("AcquireNextImageKHR timed out for duplicate frame; skipping injection this present");
        return false;
    }
    if (acquire_result != VK_SUCCESS && acquire_result != VK_SUBOPTIMAL_KHR) {
        log_warn("AcquireNextImageKHR failed for duplicate frame: " + std::to_string(acquire_result));
        return false;
    }

    const uint32_t source_index = present_info->pImageIndices[0];
    if (source_index >= state.images.size() || generated_image_index >= state.images.size()) {
        refresh_swapchain_images(state, device_info.dispatch);
        if (source_index >= state.images.size() || generated_image_index >= state.images.size()) {
            log_warn("copy mode image index out of bounds after refresh");
            return false;
        }
    }
    if (generated_image_index == source_index) {
        log_warn("duplicate frame acquire returned current source image index; skipping injection");
        return false;
    }

    const VkImage source_image = state.images[source_index];
    const VkImage generated_image = state.images[generated_image_index];

    if (device_info.dispatch.ResetCommandPool(device_info.device, state.inject.command_pool, 0) != VK_SUCCESS) {
        log_warn("ResetCommandPool failed in copy mode");
        return false;
    }

    VkCommandBufferBeginInfo begin_info{};
    begin_info.sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO;
    begin_info.flags = VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT;
    if (device_info.dispatch.BeginCommandBuffer(state.inject.command_buffer, &begin_info) != VK_SUCCESS) {
        log_warn("BeginCommandBuffer failed in copy mode");
        return false;
    }

    VkImageMemoryBarrier barriers_to_copy[2] = {
        image_barrier(source_image, VK_ACCESS_MEMORY_READ_BIT, VK_ACCESS_TRANSFER_READ_BIT,
            VK_IMAGE_LAYOUT_PRESENT_SRC_KHR, VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL),
        image_barrier(generated_image, 0, VK_ACCESS_TRANSFER_WRITE_BIT,
            VK_IMAGE_LAYOUT_UNDEFINED, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL),
    };
    device_info.dispatch.CmdPipelineBarrier(
        state.inject.command_buffer,
        VK_PIPELINE_STAGE_ALL_COMMANDS_BIT,
        VK_PIPELINE_STAGE_TRANSFER_BIT,
        0,
        0, nullptr,
        0, nullptr,
        2, barriers_to_copy);

    VkImageCopy copy_region{};
    copy_region.srcSubresource.aspectMask = VK_IMAGE_ASPECT_COLOR_BIT;
    copy_region.srcSubresource.layerCount = 1;
    copy_region.dstSubresource.aspectMask = VK_IMAGE_ASPECT_COLOR_BIT;
    copy_region.dstSubresource.layerCount = 1;
    copy_region.extent.width = state.extent.width;
    copy_region.extent.height = state.extent.height;
    copy_region.extent.depth = 1;
    device_info.dispatch.CmdCopyImage(
        state.inject.command_buffer,
        source_image,
        VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL,
        generated_image,
        VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL,
        1,
        &copy_region);

    VkImageMemoryBarrier barriers_to_present[2] = {
        image_barrier(source_image, VK_ACCESS_TRANSFER_READ_BIT, VK_ACCESS_MEMORY_READ_BIT,
            VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL, VK_IMAGE_LAYOUT_PRESENT_SRC_KHR),
        image_barrier(generated_image, VK_ACCESS_TRANSFER_WRITE_BIT, VK_ACCESS_MEMORY_READ_BIT,
            VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, VK_IMAGE_LAYOUT_PRESENT_SRC_KHR),
    };
    device_info.dispatch.CmdPipelineBarrier(
        state.inject.command_buffer,
        VK_PIPELINE_STAGE_TRANSFER_BIT,
        VK_PIPELINE_STAGE_BOTTOM_OF_PIPE_BIT,
        0,
        0, nullptr,
        0, nullptr,
        2, barriers_to_present);

    if (device_info.dispatch.EndCommandBuffer(state.inject.command_buffer) != VK_SUCCESS) {
        log_warn("EndCommandBuffer failed in copy mode");
        return false;
    }

    std::vector<VkSemaphore> wait_semaphores;
    wait_semaphores.reserve(present_info->waitSemaphoreCount + 1);
    std::vector<VkPipelineStageFlags> wait_stages;
    wait_stages.reserve(present_info->waitSemaphoreCount + 1);
    for (uint32_t i = 0; i < present_info->waitSemaphoreCount; ++i) {
        wait_semaphores.push_back(present_info->pWaitSemaphores[i]);
        wait_stages.push_back(VK_PIPELINE_STAGE_TRANSFER_BIT);
    }
    wait_semaphores.push_back(state.inject.acquire_semaphore);
    wait_stages.push_back(VK_PIPELINE_STAGE_TRANSFER_BIT);

    VkSemaphore signal_semaphores[2] = {
        state.inject.ready_original_semaphore,
        state.inject.ready_generated_semaphore,
    };

    VkSubmitInfo submit_info{};
    submit_info.sType = VK_STRUCTURE_TYPE_SUBMIT_INFO;
    submit_info.waitSemaphoreCount = static_cast<uint32_t>(wait_semaphores.size());
    submit_info.pWaitSemaphores = wait_semaphores.data();
    submit_info.pWaitDstStageMask = wait_stages.data();
    submit_info.commandBufferCount = 1;
    submit_info.pCommandBuffers = &state.inject.command_buffer;
    submit_info.signalSemaphoreCount = 2;
    submit_info.pSignalSemaphores = signal_semaphores;

    const bool first_success = !state.injection_works;
    if (device_info.dispatch.ResetFences(device_info.device, 1, &state.inject.submit_fence) != VK_SUCCESS) {
        log_warn("ResetFences failed for submit fence before copy QueueSubmit");
        return false;
    }
    const VkResult submit_result = device_info.dispatch.QueueSubmit(queue, 1, &submit_info, state.inject.submit_fence);
    if (submit_result != VK_SUCCESS) {
        log_warn("QueueSubmit failed for duplicate frame: " + std::to_string(submit_result));
        return false;
    }

    VkPresentInfoKHR original_present = *present_info;
    original_present.waitSemaphoreCount = 1;
    original_present.pWaitSemaphores = &state.inject.ready_original_semaphore;
    const VkResult original_result = device_info.dispatch.QueuePresentKHR(queue, &original_present);
    if (original_result != VK_SUCCESS && original_result != VK_SUBOPTIMAL_KHR) {
        log_warn("original QueuePresentKHR failed in copy mode: " + std::to_string(original_result));
        return false;
    }

    VkPresentInfoKHR generated_present{};
    generated_present.sType = VK_STRUCTURE_TYPE_PRESENT_INFO_KHR;
    generated_present.waitSemaphoreCount = 1;
    generated_present.pWaitSemaphores = &state.inject.ready_generated_semaphore;
    generated_present.swapchainCount = 1;
    generated_present.pSwapchains = &state.handle;
    generated_present.pImageIndices = &generated_image_index;
    const VkResult generated_result = device_info.dispatch.QueuePresentKHR(queue, &generated_present);
    if (generated_result != VK_SUCCESS && generated_result != VK_SUBOPTIMAL_KHR) {
        log_warn("generated QueuePresentKHR failed in copy mode: " + std::to_string(generated_result));
        return false;
    }

    if (device_info.dispatch.QueueWaitIdle) {
        const VkResult wait_idle_result = device_info.dispatch.QueueWaitIdle(queue);
        if (wait_idle_result != VK_SUCCESS) {
            log_warn("QueueWaitIdle failed in copy mode: " + std::to_string(wait_idle_result));
            return false;
        }
    }

    state.injection_works = true;
    state.generated_present_count++;
    if (first_success) {
        log_info("first duplicated-frame present succeeded");
    }
    if ((state.generated_present_count <= 5) || (state.generated_present_count % 60 == 0)) {
        log_info(
            std::string("duplicated frame present=") + std::to_string(state.generated_present_count)
            + "; sourceImageIndex=" + std::to_string(source_index)
            + "; generatedImageIndex=" + std::to_string(generated_image_index));
    }
    return true;
}

bool try_present_history_copy_frame(SwapchainState& state, const DeviceInfo& device_info, const QueueInfo& queue_info, VkQueue queue,
        const VkPresentInfoKHR* present_info) {
    if (!init_inject_resources(state, device_info, queue_info)) {
        return false;
    }
    if (!present_info || present_info->swapchainCount != 1 || !device_info.dispatch.CmdCopyImage) {
        return false;
    }
    if (!ensure_history_image(state, device_info)) {
        return false;
    }

    const VkResult prior_submit_wait = device_info.dispatch.WaitForFences(
        device_info.device,
        1,
        &state.inject.submit_fence,
        VK_TRUE,
        5'000'000'000ULL);
    if (prior_submit_wait != VK_SUCCESS) {
        log_warn("WaitForFences failed for submit fence: " + std::to_string(prior_submit_wait));
        return false;
    }

    const uint32_t source_index = present_info->pImageIndices[0];
    if (source_index >= state.images.size()) {
        refresh_swapchain_images(state, device_info.dispatch);
        if (source_index >= state.images.size()) {
            log_warn("history-copy source image index out of bounds after refresh");
            return false;
        }
    }
    const VkImage source_image = state.images[source_index];

    const bool have_generated = state.history_valid;
    uint32_t generated_image_index = 0;
    if (have_generated) {
        const VkResult acquire_result = device_info.dispatch.AcquireNextImageKHR(
            device_info.device,
            state.handle,
            20'000'000ULL,
            state.inject.acquire_semaphore,
            VK_NULL_HANDLE,
            &generated_image_index);
        if (acquire_result == VK_TIMEOUT || acquire_result == VK_NOT_READY) {
            log_warn("AcquireNextImageKHR timed out for history-copy frame; skipping injection this present");
            return false;
        }
        if (acquire_result != VK_SUCCESS && acquire_result != VK_SUBOPTIMAL_KHR) {
            log_warn("AcquireNextImageKHR failed for history-copy frame: " + std::to_string(acquire_result));
            return false;
        }
        if (generated_image_index >= state.images.size()) {
            refresh_swapchain_images(state, device_info.dispatch);
            if (generated_image_index >= state.images.size()) {
                log_warn("history-copy generated image index out of bounds after refresh");
                return false;
            }
        }
    }

    if (device_info.dispatch.ResetCommandPool(device_info.device, state.inject.command_pool, 0) != VK_SUCCESS) {
        log_warn("ResetCommandPool failed in history-copy mode");
        return false;
    }

    VkCommandBufferBeginInfo begin_info{};
    begin_info.sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO;
    begin_info.flags = VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT;
    if (device_info.dispatch.BeginCommandBuffer(state.inject.command_buffer, &begin_info) != VK_SUCCESS) {
        log_warn("BeginCommandBuffer failed in history-copy mode");
        return false;
    }

    std::vector<VkImageMemoryBarrier> barriers_before;
    barriers_before.reserve(3);
    barriers_before.push_back(image_barrier(source_image, VK_ACCESS_MEMORY_READ_BIT, VK_ACCESS_TRANSFER_READ_BIT,
        VK_IMAGE_LAYOUT_PRESENT_SRC_KHR, VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL));
    if (have_generated) {
        barriers_before.push_back(image_barrier(state.history_image, VK_ACCESS_MEMORY_READ_BIT, VK_ACCESS_TRANSFER_READ_BIT,
            VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL, VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL));
        barriers_before.push_back(image_barrier(state.images[generated_image_index], 0, VK_ACCESS_TRANSFER_WRITE_BIT,
            VK_IMAGE_LAYOUT_UNDEFINED, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL));
    }
    device_info.dispatch.CmdPipelineBarrier(
        state.inject.command_buffer,
        VK_PIPELINE_STAGE_ALL_COMMANDS_BIT,
        VK_PIPELINE_STAGE_TRANSFER_BIT,
        0,
        0, nullptr,
        0, nullptr,
        static_cast<uint32_t>(barriers_before.size()),
        barriers_before.data());

    if (have_generated) {
        VkImageCopy previous_copy{};
        previous_copy.srcSubresource.aspectMask = VK_IMAGE_ASPECT_COLOR_BIT;
        previous_copy.srcSubresource.layerCount = 1;
        previous_copy.dstSubresource.aspectMask = VK_IMAGE_ASPECT_COLOR_BIT;
        previous_copy.dstSubresource.layerCount = 1;
        previous_copy.extent.width = state.extent.width;
        previous_copy.extent.height = state.extent.height;
        previous_copy.extent.depth = 1;
        device_info.dispatch.CmdCopyImage(
            state.inject.command_buffer,
            state.history_image,
            VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL,
            state.images[generated_image_index],
            VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL,
            1,
            &previous_copy);
    }

    VkImageMemoryBarrier history_to_dst = image_barrier(
        state.history_image,
        have_generated ? VK_ACCESS_TRANSFER_READ_BIT : 0,
        VK_ACCESS_TRANSFER_WRITE_BIT,
        state.history_valid ? VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL : VK_IMAGE_LAYOUT_UNDEFINED,
        VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL);
    device_info.dispatch.CmdPipelineBarrier(
        state.inject.command_buffer,
        VK_PIPELINE_STAGE_TRANSFER_BIT,
        VK_PIPELINE_STAGE_TRANSFER_BIT,
        0,
        0, nullptr,
        0, nullptr,
        1,
        &history_to_dst);

    VkImageCopy current_to_history{};
    current_to_history.srcSubresource.aspectMask = VK_IMAGE_ASPECT_COLOR_BIT;
    current_to_history.srcSubresource.layerCount = 1;
    current_to_history.dstSubresource.aspectMask = VK_IMAGE_ASPECT_COLOR_BIT;
    current_to_history.dstSubresource.layerCount = 1;
    current_to_history.extent.width = state.extent.width;
    current_to_history.extent.height = state.extent.height;
    current_to_history.extent.depth = 1;
    device_info.dispatch.CmdCopyImage(
        state.inject.command_buffer,
        source_image,
        VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL,
        state.history_image,
        VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL,
        1,
        &current_to_history);

    std::vector<VkImageMemoryBarrier> barriers_after;
    barriers_after.reserve(3);
    barriers_after.push_back(image_barrier(source_image, VK_ACCESS_TRANSFER_READ_BIT, VK_ACCESS_MEMORY_READ_BIT,
        VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL, VK_IMAGE_LAYOUT_PRESENT_SRC_KHR));
    barriers_after.push_back(image_barrier(state.history_image, VK_ACCESS_TRANSFER_WRITE_BIT, VK_ACCESS_MEMORY_READ_BIT,
        VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL));
    if (have_generated) {
        barriers_after.push_back(image_barrier(state.images[generated_image_index], VK_ACCESS_TRANSFER_WRITE_BIT, VK_ACCESS_MEMORY_READ_BIT,
            VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, VK_IMAGE_LAYOUT_PRESENT_SRC_KHR));
    }
    device_info.dispatch.CmdPipelineBarrier(
        state.inject.command_buffer,
        VK_PIPELINE_STAGE_TRANSFER_BIT,
        VK_PIPELINE_STAGE_BOTTOM_OF_PIPE_BIT,
        0,
        0, nullptr,
        0, nullptr,
        static_cast<uint32_t>(barriers_after.size()),
        barriers_after.data());

    if (device_info.dispatch.EndCommandBuffer(state.inject.command_buffer) != VK_SUCCESS) {
        log_warn("EndCommandBuffer failed in history-copy mode");
        return false;
    }

    std::vector<VkSemaphore> wait_semaphores;
    std::vector<VkPipelineStageFlags> wait_stages;
    wait_semaphores.reserve(present_info->waitSemaphoreCount + (have_generated ? 1u : 0u));
    wait_stages.reserve(present_info->waitSemaphoreCount + (have_generated ? 1u : 0u));
    for (uint32_t i = 0; i < present_info->waitSemaphoreCount; ++i) {
        wait_semaphores.push_back(present_info->pWaitSemaphores[i]);
        wait_stages.push_back(VK_PIPELINE_STAGE_TRANSFER_BIT);
    }
    if (have_generated) {
        wait_semaphores.push_back(state.inject.acquire_semaphore);
        wait_stages.push_back(VK_PIPELINE_STAGE_TRANSFER_BIT);
    }

    std::vector<VkSemaphore> signal_semaphores;
    signal_semaphores.push_back(state.inject.ready_original_semaphore);
    if (have_generated) {
        signal_semaphores.push_back(state.inject.ready_generated_semaphore);
    }

    VkSubmitInfo submit_info{};
    submit_info.sType = VK_STRUCTURE_TYPE_SUBMIT_INFO;
    submit_info.waitSemaphoreCount = static_cast<uint32_t>(wait_semaphores.size());
    submit_info.pWaitSemaphores = wait_semaphores.empty() ? nullptr : wait_semaphores.data();
    submit_info.pWaitDstStageMask = wait_stages.empty() ? nullptr : wait_stages.data();
    submit_info.commandBufferCount = 1;
    submit_info.pCommandBuffers = &state.inject.command_buffer;
    submit_info.signalSemaphoreCount = static_cast<uint32_t>(signal_semaphores.size());
    submit_info.pSignalSemaphores = signal_semaphores.data();

    if (device_info.dispatch.ResetFences(device_info.device, 1, &state.inject.submit_fence) != VK_SUCCESS) {
        log_warn("ResetFences failed for submit fence before history-copy QueueSubmit");
        return false;
    }
    const VkResult submit_result = device_info.dispatch.QueueSubmit(queue, 1, &submit_info, state.inject.submit_fence);
    if (submit_result != VK_SUCCESS) {
        log_warn("QueueSubmit failed for history-copy frame: " + std::to_string(submit_result));
        return false;
    }

    const bool first_success = !state.injection_works;
    if (have_generated) {
        VkPresentInfoKHR generated_present{};
        generated_present.sType = VK_STRUCTURE_TYPE_PRESENT_INFO_KHR;
        generated_present.waitSemaphoreCount = 1;
        generated_present.pWaitSemaphores = &state.inject.ready_generated_semaphore;
        generated_present.swapchainCount = 1;
        generated_present.pSwapchains = &state.handle;
        generated_present.pImageIndices = &generated_image_index;
        const VkResult generated_result = device_info.dispatch.QueuePresentKHR(queue, &generated_present);
        if (generated_result != VK_SUCCESS && generated_result != VK_SUBOPTIMAL_KHR) {
            log_warn("generated QueuePresentKHR failed in history-copy mode: " + std::to_string(generated_result));
            return false;
        }
    }

    VkPresentInfoKHR original_present = *present_info;
    original_present.waitSemaphoreCount = 1;
    original_present.pWaitSemaphores = &state.inject.ready_original_semaphore;
    const VkResult original_result = device_info.dispatch.QueuePresentKHR(queue, &original_present);
    if (original_result != VK_SUCCESS && original_result != VK_SUBOPTIMAL_KHR) {
        log_warn("original QueuePresentKHR failed in history-copy mode: " + std::to_string(original_result));
        return false;
    }

    if (device_info.dispatch.QueueWaitIdle) {
        const VkResult wait_idle_result = device_info.dispatch.QueueWaitIdle(queue);
        if (wait_idle_result != VK_SUCCESS) {
            log_warn("QueueWaitIdle failed in history-copy mode: " + std::to_string(wait_idle_result));
            return false;
        }
    }

    state.history_valid = true;
    state.injection_works = state.injection_works || have_generated;
    if (have_generated) {
        state.generated_present_count++;
        if (first_success) {
            log_info("first previous-frame insertion present succeeded");
        }
        if ((state.generated_present_count <= 5) || (state.generated_present_count % 60 == 0)) {
            log_info(
                std::string("history-copy generated frame present=") + std::to_string(state.generated_present_count)
                + "; previousFrameSourceStored=1"
                + "; generatedImageIndex=" + std::to_string(generated_image_index)
                + "; currentImageIndex=" + std::to_string(source_index));
        }
    } else {
        log_info("history-copy primed previous frame history");
    }

    return true;
}

bool try_present_clear_frame(SwapchainState& state, const DeviceInfo& device_info, const QueueInfo& queue_info, VkQueue queue) {
    if (!init_inject_resources(state, device_info, queue_info)) {
        return false;
    }

    const VkResult prior_submit_wait = device_info.dispatch.WaitForFences(
        device_info.device,
        1,
        &state.inject.submit_fence,
        VK_TRUE,
        5'000'000'000ULL);
    if (prior_submit_wait != VK_SUCCESS) {
        log_warn("WaitForFences failed for submit fence: " + std::to_string(prior_submit_wait));
        return false;
    }

    uint32_t generated_image_index = 0;
    const VkResult acquire_result = device_info.dispatch.AcquireNextImageKHR(
        device_info.device,
        state.handle,
        20'000'000ULL,
        state.inject.acquire_semaphore,
        VK_NULL_HANDLE,
        &generated_image_index);

    if (acquire_result == VK_TIMEOUT || acquire_result == VK_NOT_READY) {
        log_warn("AcquireNextImageKHR timed out for generated frame; skipping injection this present");
        return false;
    }
    if (acquire_result != VK_SUCCESS && acquire_result != VK_SUBOPTIMAL_KHR) {
        log_warn("AcquireNextImageKHR failed for generated frame: " + std::to_string(acquire_result));
        return false;
    }

    if (generated_image_index >= state.images.size()) {
        refresh_swapchain_images(state, device_info.dispatch);
        if (generated_image_index >= state.images.size()) {
            log_warn("generated image index out of bounds after refresh");
            return false;
        }
    }

    const VkImage generated_image = state.images[generated_image_index];
    if (device_info.dispatch.ResetCommandPool(device_info.device, state.inject.command_pool, 0) != VK_SUCCESS) {
        log_warn("ResetCommandPool failed");
        return false;
    }

    VkCommandBufferBeginInfo begin_info{};
    begin_info.sType = VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO;
    begin_info.flags = VK_COMMAND_BUFFER_USAGE_ONE_TIME_SUBMIT_BIT;
    if (device_info.dispatch.BeginCommandBuffer(state.inject.command_buffer, &begin_info) != VK_SUCCESS) {
        log_warn("BeginCommandBuffer failed");
        return false;
    }

    auto to_transfer_dst = image_barrier(
        generated_image,
        0,
        VK_ACCESS_TRANSFER_WRITE_BIT,
        VK_IMAGE_LAYOUT_UNDEFINED,
        VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL);
    device_info.dispatch.CmdPipelineBarrier(
        state.inject.command_buffer,
        VK_PIPELINE_STAGE_TOP_OF_PIPE_BIT,
        VK_PIPELINE_STAGE_TRANSFER_BIT,
        0,
        0, nullptr,
        0, nullptr,
        1, &to_transfer_dst);

    const float pulse = (state.present_count % 120u) < 60u ? 0.85f : 0.15f;
    VkClearColorValue clear_color{};
    clear_color.float32[0] = 0.0f;
    clear_color.float32[1] = pulse;
    clear_color.float32[2] = 0.0f;
    clear_color.float32[3] = 1.0f;

    VkImageSubresourceRange range{};
    range.aspectMask = VK_IMAGE_ASPECT_COLOR_BIT;
    range.baseMipLevel = 0;
    range.levelCount = 1;
    range.baseArrayLayer = 0;
    range.layerCount = 1;
    device_info.dispatch.CmdClearColorImage(
        state.inject.command_buffer,
        generated_image,
        VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL,
        &clear_color,
        1,
        &range);

    auto to_present = image_barrier(
        generated_image,
        VK_ACCESS_TRANSFER_WRITE_BIT,
        VK_ACCESS_MEMORY_READ_BIT,
        VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL,
        VK_IMAGE_LAYOUT_PRESENT_SRC_KHR);
    device_info.dispatch.CmdPipelineBarrier(
        state.inject.command_buffer,
        VK_PIPELINE_STAGE_TRANSFER_BIT,
        VK_PIPELINE_STAGE_BOTTOM_OF_PIPE_BIT,
        0,
        0, nullptr,
        0, nullptr,
        1, &to_present);

    if (device_info.dispatch.EndCommandBuffer(state.inject.command_buffer) != VK_SUCCESS) {
        log_warn("EndCommandBuffer failed");
        return false;
    }

    const VkPipelineStageFlags wait_stage = VK_PIPELINE_STAGE_TRANSFER_BIT;
    VkSubmitInfo submit_info{};
    submit_info.sType = VK_STRUCTURE_TYPE_SUBMIT_INFO;
    submit_info.waitSemaphoreCount = 1;
    submit_info.pWaitSemaphores = &state.inject.acquire_semaphore;
    submit_info.pWaitDstStageMask = &wait_stage;
    submit_info.commandBufferCount = 1;
    submit_info.pCommandBuffers = &state.inject.command_buffer;

    const bool first_success = !state.injection_works;
    if (device_info.dispatch.ResetFences(device_info.device, 1, &state.inject.submit_fence) != VK_SUCCESS) {
        log_warn("ResetFences failed for submit fence before QueueSubmit");
        return false;
    }
    const VkResult submit_result = device_info.dispatch.QueueSubmit(queue, 1, &submit_info, state.inject.submit_fence);
    if (submit_result != VK_SUCCESS) {
        log_warn("QueueSubmit failed for generated frame: " + std::to_string(submit_result));
        return false;
    }

    const VkResult submit_wait = device_info.dispatch.WaitForFences(device_info.device, 1, &state.inject.submit_fence, VK_TRUE, 5'000'000'000ULL);
    if (submit_wait != VK_SUCCESS) {
        log_warn("WaitForFences failed after generated frame submit: " + std::to_string(submit_wait));
        return false;
    }

    VkPresentInfoKHR generated_present{};
    generated_present.sType = VK_STRUCTURE_TYPE_PRESENT_INFO_KHR;
    generated_present.swapchainCount = 1;
    generated_present.pSwapchains = &state.handle;
    generated_present.pImageIndices = &generated_image_index;
    const VkResult generated_present_result = device_info.dispatch.QueuePresentKHR(queue, &generated_present);
    if (generated_present_result != VK_SUCCESS && generated_present_result != VK_SUBOPTIMAL_KHR) {
        log_warn("generated QueuePresentKHR failed: " + std::to_string(generated_present_result));
        return false;
    }

    state.injection_works = true;
    state.generated_present_count++;
    if (first_success) {
        log_info("first generated clear-frame present succeeded");
    }
    if ((state.generated_present_count <= 5) || (state.generated_present_count % 60 == 0)) {
        log_info(
            std::string("generated frame present=") + std::to_string(state.generated_present_count)
            + "; swapchainImageIndex=" + std::to_string(generated_image_index));
    }
    return true;
}

VkLayerInstanceCreateInfo* find_instance_layer_link(const VkInstanceCreateInfo* create_info) {
    auto* layer_info = reinterpret_cast<VkLayerInstanceCreateInfo*>(const_cast<void*>(create_info ? create_info->pNext : nullptr));
    while (layer_info && (layer_info->sType != VK_STRUCTURE_TYPE_LOADER_INSTANCE_CREATE_INFO || layer_info->function != VK_LAYER_LINK_INFO)) {
        layer_info = reinterpret_cast<VkLayerInstanceCreateInfo*>(const_cast<void*>(layer_info->pNext));
    }
    return layer_info;
}

VkLayerDeviceCreateInfo* find_device_layer_link(const VkDeviceCreateInfo* create_info) {
    auto* layer_info = reinterpret_cast<VkLayerDeviceCreateInfo*>(const_cast<void*>(create_info ? create_info->pNext : nullptr));
    while (layer_info && (layer_info->sType != VK_STRUCTURE_TYPE_LOADER_DEVICE_CREATE_INFO || layer_info->function != VK_LAYER_LINK_INFO)) {
        layer_info = reinterpret_cast<VkLayerDeviceCreateInfo*>(const_cast<void*>(layer_info->pNext));
    }
    return layer_info;
}

PFN_vkVoidFunction get_intercepted_proc_addr(const char* name);

VkResult VKAPI_CALL layer_create_instance(const VkInstanceCreateInfo* create_info,
        const VkAllocationCallbacks* allocator,
        VkInstance* instance) {
    const Mode mode = current_mode();

    VkLayerInstanceCreateInfo* layer_info = find_instance_layer_link(create_info);
    if (!layer_info || !layer_info->u.pLayerInfo || !layer_info->u.pLayerInfo->pfnNextGetInstanceProcAddr) {
        log_error("vkCreateInstance: failed to find next layer link info");
        return VK_ERROR_INITIALIZATION_FAILED;
    }

    PFN_vkGetInstanceProcAddr next_gipa = layer_info->u.pLayerInfo->pfnNextGetInstanceProcAddr;
    layer_info->u.pLayerInfo = layer_info->u.pLayerInfo->pNext;

    auto next_create_instance = reinterpret_cast<PFN_vkCreateInstance>(next_gipa(VK_NULL_HANDLE, "vkCreateInstance"));
    if (!next_create_instance) {
        log_error("vkCreateInstance: next vkCreateInstance lookup failed");
        return VK_ERROR_INITIALIZATION_FAILED;
    }

    const VkResult result = next_create_instance(create_info, allocator, instance);
    if (result != VK_SUCCESS) {
        log_warn("vkCreateInstance returned " + std::to_string(result));
        return result;
    }

    InstanceDispatch dispatch{};
    fill_instance_dispatch(*instance, next_gipa, dispatch);

    {
        std::lock_guard<std::mutex> lock(g_mutex);
        g_instance_dispatch[get_key(*instance)] = dispatch;
        g_instance_map[get_key(*instance)] = *instance;
    }

    std::string message = std::string("vkCreateInstance ok; mode=") + mode_name(mode);
    if (create_info && create_info->pApplicationInfo) {
        if (create_info->pApplicationInfo->pApplicationName) {
            message += std::string("; app=") + create_info->pApplicationInfo->pApplicationName;
        }
        if (create_info->pApplicationInfo->pEngineName) {
            message += std::string("; engine=") + create_info->pApplicationInfo->pEngineName;
        }
        message += std::string("; apiVersion=")
            + std::to_string(VK_VERSION_MAJOR(create_info->pApplicationInfo->apiVersion))
            + "." + std::to_string(VK_VERSION_MINOR(create_info->pApplicationInfo->apiVersion))
            + "." + std::to_string(VK_VERSION_PATCH(create_info->pApplicationInfo->apiVersion));
    }
    log_info(message);
    return VK_SUCCESS;
}

void VKAPI_CALL layer_destroy_instance(VkInstance instance, const VkAllocationCallbacks* allocator) {
    if (!instance) {
        return;
    }

    InstanceDispatch dispatch{};
    {
        std::lock_guard<std::mutex> lock(g_mutex);
        auto it = g_instance_dispatch.find(get_key(instance));
        if (it != g_instance_dispatch.end()) {
            dispatch = it->second;
            g_instance_dispatch.erase(it);
        }
        g_instance_map.erase(get_key(instance));
    }

    log_info("vkDestroyInstance");
    if (dispatch.DestroyInstance) {
        dispatch.DestroyInstance(instance, allocator);
    }
}

VkResult VKAPI_CALL layer_create_device(VkPhysicalDevice physical_device,
        const VkDeviceCreateInfo* create_info,
        const VkAllocationCallbacks* allocator,
        VkDevice* device) {
    VkLayerDeviceCreateInfo* layer_info = find_device_layer_link(create_info);
    if (!layer_info || !layer_info->u.pLayerInfo) {
        log_error("vkCreateDevice: failed to find next layer link info");
        return VK_ERROR_INITIALIZATION_FAILED;
    }

    PFN_vkGetInstanceProcAddr next_gipa = layer_info->u.pLayerInfo->pfnNextGetInstanceProcAddr;
    PFN_vkGetDeviceProcAddr next_gdpa = layer_info->u.pLayerInfo->pfnNextGetDeviceProcAddr;
    layer_info->u.pLayerInfo = layer_info->u.pLayerInfo->pNext;

    auto next_create_device = reinterpret_cast<PFN_vkCreateDevice>(next_gipa(VK_NULL_HANDLE, "vkCreateDevice"));
    if (!next_create_device) {
        log_error("vkCreateDevice: next vkCreateDevice lookup failed");
        return VK_ERROR_INITIALIZATION_FAILED;
    }

    const VkResult result = next_create_device(physical_device, create_info, allocator, device);
    if (result != VK_SUCCESS) {
        log_warn("vkCreateDevice returned " + std::to_string(result));
        return result;
    }

    InstanceDispatch instance_dispatch{};
    VkInstance instance = VK_NULL_HANDLE;
    {
        std::lock_guard<std::mutex> lock(g_mutex);
        if (const auto it = g_instance_dispatch.find(get_key(physical_device)); it != g_instance_dispatch.end()) {
            instance_dispatch = it->second;
        }
        if (const auto it = g_instance_map.find(get_key(physical_device)); it != g_instance_map.end()) {
            instance = it->second;
        }
    }

    DeviceDispatch device_dispatch{};
    fill_device_dispatch(*device, next_gdpa, device_dispatch);

    DeviceInfo device_info{};
    device_info.instance = instance;
    device_info.physical_device = physical_device;
    device_info.device = *device;
    device_info.instance_dispatch = instance_dispatch;
    device_info.dispatch = device_dispatch;

    if (instance_dispatch.GetPhysicalDeviceQueueFamilyProperties && device_dispatch.GetDeviceQueue) {
        uint32_t queue_family_count = 0;
        instance_dispatch.GetPhysicalDeviceQueueFamilyProperties(physical_device, &queue_family_count, nullptr);
        std::vector<VkQueueFamilyProperties> queue_families(queue_family_count);
        instance_dispatch.GetPhysicalDeviceQueueFamilyProperties(physical_device, &queue_family_count, queue_families.data());

        for (uint32_t i = 0; i < create_info->queueCreateInfoCount; ++i) {
            const VkDeviceQueueCreateInfo& queue_create_info = create_info->pQueueCreateInfos[i];
            const bool supports_graphics = queue_create_info.queueFamilyIndex < queue_families.size()
                && (queue_families[queue_create_info.queueFamilyIndex].queueFlags & VK_QUEUE_GRAPHICS_BIT) != 0;
            const bool supports_transfer = queue_create_info.queueFamilyIndex < queue_families.size()
                && (queue_families[queue_create_info.queueFamilyIndex].queueFlags & VK_QUEUE_TRANSFER_BIT) != 0;

            for (uint32_t queue_index = 0; queue_index < queue_create_info.queueCount; ++queue_index) {
                VkQueue queue = VK_NULL_HANDLE;
                device_dispatch.GetDeviceQueue(*device, queue_create_info.queueFamilyIndex, queue_index, &queue);
                if (!queue) {
                    continue;
                }

                remember_queue(
                    queue,
                    *device,
                    queue_create_info.queueFamilyIndex,
                    queue_index,
                    supports_graphics,
                    supports_transfer);
            }
        }
    }

    VkPhysicalDeviceProperties properties{};
    if (instance_dispatch.GetPhysicalDeviceProperties) {
        instance_dispatch.GetPhysicalDeviceProperties(physical_device, &properties);
    }

    {
        std::lock_guard<std::mutex> lock(g_mutex);
        g_device_map[get_key(*device)] = device_info;
    }

    log_info(std::string("vkCreateDevice ok; gpu=") + properties.deviceName);
    return VK_SUCCESS;
}

void VKAPI_CALL layer_destroy_device(VkDevice device, const VkAllocationCallbacks* allocator) {
    if (!device) {
        return;
    }

    DeviceInfo device_info{};
    bool found = false;
    {
        std::lock_guard<std::mutex> lock(g_mutex);
        if (const auto it = g_device_map.find(get_key(device)); it != g_device_map.end()) {
            device_info = it->second;
            g_device_map.erase(it);
            found = true;
        }

        for (auto it = g_queue_map.begin(); it != g_queue_map.end();) {
            if (it->second.device == device) {
                it = g_queue_map.erase(it);
            } else {
                ++it;
            }
        }

        for (auto it = g_swapchains.begin(); it != g_swapchains.end();) {
            if (it->second.device == device) {
                destroy_swapchain_resources(device_info, it->second);
                it = g_swapchains.erase(it);
            } else {
                ++it;
            }
        }
    }

    log_info("vkDestroyDevice");
    if (found && device_info.dispatch.DestroyDevice) {
        device_info.dispatch.DestroyDevice(device, allocator);
    }
}

void VKAPI_CALL layer_get_device_queue(VkDevice device, uint32_t queue_family_index, uint32_t queue_index, VkQueue* queue) {
    DeviceInfo device_info{};
    {
        std::lock_guard<std::mutex> lock(g_mutex);
        if (const auto it = g_device_map.find(get_key(device)); it != g_device_map.end()) {
            device_info = it->second;
        }
    }

    if (!device_info.dispatch.GetDeviceQueue) {
        return;
    }

    device_info.dispatch.GetDeviceQueue(device, queue_family_index, queue_index, queue);

    bool supports_graphics = false;
    bool supports_transfer = false;
    if (device_info.instance_dispatch.GetPhysicalDeviceQueueFamilyProperties && device_info.physical_device != VK_NULL_HANDLE) {
        uint32_t queue_family_count = 0;
        device_info.instance_dispatch.GetPhysicalDeviceQueueFamilyProperties(device_info.physical_device, &queue_family_count, nullptr);
        std::vector<VkQueueFamilyProperties> queue_families(queue_family_count);
        device_info.instance_dispatch.GetPhysicalDeviceQueueFamilyProperties(device_info.physical_device, &queue_family_count, queue_families.data());
        if (queue_family_index < queue_families.size()) {
            supports_graphics = (queue_families[queue_family_index].queueFlags & VK_QUEUE_GRAPHICS_BIT) != 0;
            supports_transfer = (queue_families[queue_family_index].queueFlags & VK_QUEUE_TRANSFER_BIT) != 0;
        }
    }
    remember_queue(*queue, device, queue_family_index, queue_index, supports_graphics, supports_transfer);
}

void VKAPI_CALL layer_get_device_queue2(VkDevice device, const VkDeviceQueueInfo2* queue_info, VkQueue* queue) {
    DeviceInfo device_info{};
    {
        std::lock_guard<std::mutex> lock(g_mutex);
        if (const auto it = g_device_map.find(get_key(device)); it != g_device_map.end()) {
            device_info = it->second;
        }
    }

    if (!device_info.dispatch.GetDeviceQueue2) {
        return;
    }

    device_info.dispatch.GetDeviceQueue2(device, queue_info, queue);

    bool supports_graphics = false;
    bool supports_transfer = false;
    if (device_info.instance_dispatch.GetPhysicalDeviceQueueFamilyProperties && device_info.physical_device != VK_NULL_HANDLE) {
        uint32_t queue_family_count = 0;
        device_info.instance_dispatch.GetPhysicalDeviceQueueFamilyProperties(device_info.physical_device, &queue_family_count, nullptr);
        std::vector<VkQueueFamilyProperties> queue_families(queue_family_count);
        device_info.instance_dispatch.GetPhysicalDeviceQueueFamilyProperties(device_info.physical_device, &queue_family_count, queue_families.data());
        if (queue_info->queueFamilyIndex < queue_families.size()) {
            supports_graphics = (queue_families[queue_info->queueFamilyIndex].queueFlags & VK_QUEUE_GRAPHICS_BIT) != 0;
            supports_transfer = (queue_families[queue_info->queueFamilyIndex].queueFlags & VK_QUEUE_TRANSFER_BIT) != 0;
        }
    }
    remember_queue(*queue, device, queue_info->queueFamilyIndex, queue_info->queueIndex, supports_graphics, supports_transfer);
}

VkResult VKAPI_CALL layer_create_swapchain_khr(VkDevice device,
        const VkSwapchainCreateInfoKHR* create_info,
        const VkAllocationCallbacks* allocator,
        VkSwapchainKHR* swapchain) {
    DeviceInfo device_info{};
    {
        std::lock_guard<std::mutex> lock(g_mutex);
        if (const auto it = g_device_map.find(get_key(device)); it != g_device_map.end()) {
            device_info = it->second;
        } else {
            log_warn("vkCreateSwapchainKHR: device not found in layer state; passing through without tracking");
            return VK_ERROR_INITIALIZATION_FAILED;
        }
    }

    VkSwapchainCreateInfoKHR modified = *create_info;
    VkSurfaceCapabilitiesKHR caps{};
    const Mode mode = current_mode();

    if (mode == Mode::ClearTest || mode == Mode::CopyTest || mode == Mode::HistoryCopyTest) {
        modified.imageUsage |= VK_IMAGE_USAGE_TRANSFER_DST_BIT;
        if (mode == Mode::CopyTest || mode == Mode::HistoryCopyTest) {
            modified.imageUsage |= VK_IMAGE_USAGE_TRANSFER_SRC_BIT;
        }

        if (device_info.instance_dispatch.GetPhysicalDeviceSurfaceCapabilitiesKHR
                && create_info->surface != VK_NULL_HANDLE
                && device_info.physical_device != VK_NULL_HANDLE) {
            const VkResult caps_result = device_info.instance_dispatch.GetPhysicalDeviceSurfaceCapabilitiesKHR(
                device_info.physical_device,
                create_info->surface,
                &caps);
            if (caps_result == VK_SUCCESS) {
                const uint32_t image_bump = (mode == Mode::CopyTest || mode == Mode::HistoryCopyTest) ? 2u : 1u;
                const uint32_t desired = create_info->minImageCount + image_bump;
                if (caps.maxImageCount == 0) {
                    modified.minImageCount = desired;
                } else {
                    modified.minImageCount = std::min(desired, caps.maxImageCount);
                }
            }
        }
    }

    const VkResult result = device_info.dispatch.CreateSwapchainKHR(device, &modified, allocator, swapchain);
    if (result != VK_SUCCESS) {
        log_warn("vkCreateSwapchainKHR failed: " + std::to_string(result));
        return result;
    }

    if (create_info->oldSwapchain) {
        std::lock_guard<std::mutex> lock(g_mutex);
        if (auto it = g_swapchains.find(create_info->oldSwapchain); it != g_swapchains.end()) {
            destroy_swapchain_resources(device_info, it->second);
            g_swapchains.erase(it);
        }
    }

    SwapchainState state{};
    state.device = device;
    state.physical_device = device_info.physical_device;
    state.surface = create_info->surface;
    state.handle = *swapchain;
    state.format = modified.imageFormat;
    state.extent = modified.imageExtent;
    state.present_mode = modified.presentMode;
    state.original_usage = create_info->imageUsage;
    state.modified_usage = modified.imageUsage;
    state.original_min_image_count = create_info->minImageCount;
    state.modified_min_image_count = modified.minImageCount;
    refresh_swapchain_images(state, device_info.dispatch);
    if (mode == Mode::HistoryCopyTest) {
        (void)ensure_history_image(state, device_info);
    }

    {
        std::lock_guard<std::mutex> lock(g_mutex);
        g_swapchains[*swapchain] = state;
    }

    log_info(
        std::string("vkCreateSwapchainKHR ok; extent=") + format_extent(modified.imageExtent)
        + "; format=" + std::to_string(static_cast<int>(modified.imageFormat))
        + "; presentMode=" + present_mode_name(modified.presentMode)
        + "; minImages=" + std::to_string(create_info->minImageCount) + "->" + std::to_string(modified.minImageCount)
        + "; usage=" + usage_flags(create_info->imageUsage) + " -> " + usage_flags(modified.imageUsage)
        + "; images=" + std::to_string(state.images.size())
        + "; mode=" + mode_name(mode));

    return result;
}

void VKAPI_CALL layer_destroy_swapchain_khr(VkDevice device, VkSwapchainKHR swapchain, const VkAllocationCallbacks* allocator) {
    DeviceInfo device_info{};
    bool have_device = false;
    {
        std::lock_guard<std::mutex> lock(g_mutex);
        if (const auto it = g_device_map.find(get_key(device)); it != g_device_map.end()) {
            device_info = it->second;
            have_device = true;
        }
        if (auto it = g_swapchains.find(swapchain); it != g_swapchains.end()) {
            if (have_device) {
                destroy_swapchain_resources(device_info, it->second);
            }
            g_swapchains.erase(it);
        }
    }

    log_info("vkDestroySwapchainKHR");
    if (have_device && device_info.dispatch.DestroySwapchainKHR) {
        device_info.dispatch.DestroySwapchainKHR(device, swapchain, allocator);
    }
}

VkResult VKAPI_CALL layer_queue_present_khr(VkQueue queue, const VkPresentInfoKHR* present_info) {
    QueueInfo queue_info{};
    DeviceInfo device_info{};
    bool have_queue = false;
    {
        std::lock_guard<std::mutex> lock(g_mutex);
        if (const auto queue_it = g_queue_map.find(queue_id(queue)); queue_it != g_queue_map.end()) {
            queue_info = queue_it->second;
            have_queue = true;
            if (const auto device_it = g_device_map.find(get_key(queue_info.device)); device_it != g_device_map.end()) {
                device_info = device_it->second;
            }
        } else if (const auto device_it = g_device_map.find(get_key(queue)); device_it != g_device_map.end()) {
            device_info = device_it->second;
        }
    }

    if (!device_info.dispatch.QueuePresentKHR || !present_info) {
        log_warn("vkQueuePresentKHR: device dispatch not available");
        return VK_ERROR_INITIALIZATION_FAILED;
    }
    if (!have_queue) {
        log_warn("vkQueuePresentKHR: queue family not tracked; using passthrough-only fallback for this queue");
    }

    const Mode mode = current_mode();

    if (present_info->swapchainCount == 1) {
        {
            std::lock_guard<std::mutex> lock(g_mutex);
            if (auto it = g_swapchains.find(present_info->pSwapchains[0]); it != g_swapchains.end()) {
                SwapchainState& state = it->second;
                state.present_count++;
                if ((state.present_count <= 5) || (state.present_count % 60 == 0)) {
                    const char* prefix = "vkQueuePresentKHR passthrough frame=";
                    if (mode == Mode::ClearTest || mode == Mode::CopyTest || mode == Mode::HistoryCopyTest) {
                        prefix = "vkQueuePresentKHR frame=";
                    }
                    log_info(
                        std::string(prefix)
                        + std::to_string(state.present_count)
                        + "; queueFamily=" + std::to_string(queue_info.family_index)
                        + "; imageIndex=" + std::to_string(present_info->pImageIndices[0])
                        + "; waitSemaphores=" + std::to_string(present_info->waitSemaphoreCount));
                }
            }
        }

        if (have_queue && mode == Mode::ClearTest) {
            const VkResult original_result = device_info.dispatch.QueuePresentKHR(queue, present_info);
            if (original_result != VK_SUCCESS && original_result != VK_SUBOPTIMAL_KHR) {
                return original_result;
            }

            std::lock_guard<std::mutex> lock(g_mutex);
            if (auto it = g_swapchains.find(present_info->pSwapchains[0]); it != g_swapchains.end()) {
                SwapchainState& state = it->second;
                state.injection_attempted = true;
                (void)try_present_clear_frame(state, device_info, queue_info, queue);
            }
            return original_result;
        }

        if (have_queue && mode == Mode::CopyTest) {
            std::lock_guard<std::mutex> lock(g_mutex);
            if (auto it = g_swapchains.find(present_info->pSwapchains[0]); it != g_swapchains.end()) {
                SwapchainState& state = it->second;
                state.injection_attempted = true;
                if (try_present_copy_frame(state, device_info, queue_info, queue, present_info)) {
                    return VK_SUCCESS;
                }
            }
        }

        if (have_queue && mode == Mode::HistoryCopyTest) {
            std::lock_guard<std::mutex> lock(g_mutex);
            if (auto it = g_swapchains.find(present_info->pSwapchains[0]); it != g_swapchains.end()) {
                SwapchainState& state = it->second;
                state.injection_attempted = true;
                if (try_present_history_copy_frame(state, device_info, queue_info, queue, present_info)) {
                    return VK_SUCCESS;
                }
            }
        }
    }

    return device_info.dispatch.QueuePresentKHR(queue, present_info);
}

PFN_vkVoidFunction get_intercepted_proc_addr(const char* name) {
    if (!name) {
        return nullptr;
    }

#define PPFG_HOOK(func) \
    if (std::strcmp(name, "vk" #func) == 0) { \
        return reinterpret_cast<PFN_vkVoidFunction>(&layer_##func); \
    }

    PPFG_HOOK(create_instance)
    PPFG_HOOK(destroy_instance)
    PPFG_HOOK(create_device)
    PPFG_HOOK(destroy_device)
    PPFG_HOOK(create_swapchain_khr)
    PPFG_HOOK(destroy_swapchain_khr)
    PPFG_HOOK(queue_present_khr)

#undef PPFG_HOOK

    return nullptr;
}

PFN_vkVoidFunction get_instance_fallback_proc_addr(VkInstance instance, const char* name) {
    std::lock_guard<std::mutex> lock(g_mutex);
    if (const auto it = g_instance_dispatch.find(get_key(instance)); it != g_instance_dispatch.end()) {
        if (it->second.GetInstanceProcAddr) {
            return it->second.GetInstanceProcAddr(instance, name);
        }
    }
    return nullptr;
}

PFN_vkVoidFunction get_device_fallback_proc_addr(VkDevice device, const char* name) {
    std::lock_guard<std::mutex> lock(g_mutex);
    if (const auto it = g_device_map.find(get_key(device)); it != g_device_map.end()) {
        if (it->second.dispatch.GetDeviceProcAddr) {
            return it->second.dispatch.GetDeviceProcAddr(device, name);
        }
    }
    return nullptr;
}

} // namespace ppfg

extern "C" {

PPFG_EXPORT VKAPI_ATTR PFN_vkVoidFunction VKAPI_CALL vkGetInstanceProcAddr(VkInstance instance, const char* name);
PPFG_EXPORT VKAPI_ATTR PFN_vkVoidFunction VKAPI_CALL vkGetDeviceProcAddr(VkDevice device, const char* name);
PPFG_EXPORT VKAPI_ATTR VkResult VKAPI_CALL vkNegotiateLoaderLayerInterfaceVersion(VkNegotiateLayerInterface* version_struct);
PPFG_EXPORT VKAPI_ATTR VkResult VKAPI_CALL vkEnumerateInstanceLayerProperties(uint32_t* property_count, VkLayerProperties* properties);
PPFG_EXPORT VKAPI_ATTR VkResult VKAPI_CALL vkEnumerateInstanceExtensionProperties(const char* layer_name, uint32_t* property_count,
        VkExtensionProperties* properties);
PPFG_EXPORT VKAPI_ATTR VkResult VKAPI_CALL vkEnumerateDeviceLayerProperties(VkPhysicalDevice physical_device, uint32_t* property_count,
        VkLayerProperties* properties);
PPFG_EXPORT VKAPI_ATTR VkResult VKAPI_CALL vkEnumerateDeviceExtensionProperties(VkPhysicalDevice physical_device, const char* layer_name,
        uint32_t* property_count, VkExtensionProperties* properties);

PPFG_EXPORT VKAPI_ATTR VkResult VKAPI_CALL vkEnumerateInstanceLayerProperties(uint32_t* property_count, VkLayerProperties* properties) {
    if (!property_count) {
        return VK_ERROR_INITIALIZATION_FAILED;
    }

    *property_count = 1;
    if (properties) {
        std::memset(properties, 0, sizeof(VkLayerProperties));
        std::strncpy(properties[0].layerName, ppfg::kLayerName, VK_MAX_EXTENSION_NAME_SIZE - 1);
        std::strncpy(properties[0].description, "Post-process frame generation MVP Vulkan layer", VK_MAX_DESCRIPTION_SIZE - 1);
        properties[0].implementationVersion = 1;
        properties[0].specVersion = VK_MAKE_API_VERSION(0, 1, 3, 250);
    }
    return VK_SUCCESS;
}

PPFG_EXPORT VKAPI_ATTR VkResult VKAPI_CALL vkEnumerateInstanceExtensionProperties(const char* layer_name, uint32_t* property_count,
        VkExtensionProperties* properties) {
    if (layer_name && std::strcmp(layer_name, ppfg::kLayerName) != 0) {
        return VK_ERROR_LAYER_NOT_PRESENT;
    }
    if (property_count) {
        *property_count = 0;
    }
    (void)properties;
    return VK_SUCCESS;
}

PPFG_EXPORT VKAPI_ATTR VkResult VKAPI_CALL vkEnumerateDeviceLayerProperties(VkPhysicalDevice physical_device, uint32_t* property_count,
        VkLayerProperties* properties) {
    (void)physical_device;
    return vkEnumerateInstanceLayerProperties(property_count, properties);
}

PPFG_EXPORT VKAPI_ATTR VkResult VKAPI_CALL vkEnumerateDeviceExtensionProperties(VkPhysicalDevice physical_device, const char* layer_name,
        uint32_t* property_count, VkExtensionProperties* properties) {
    if (layer_name && std::strcmp(layer_name, ppfg::kLayerName) == 0) {
        if (property_count) {
            *property_count = 0;
        }
        (void)physical_device;
        (void)properties;
        return VK_SUCCESS;
    }

    std::lock_guard<std::mutex> lock(ppfg::g_mutex);
    if (const auto it = ppfg::g_instance_dispatch.find(ppfg::get_key(physical_device)); it != ppfg::g_instance_dispatch.end()) {
        if (it->second.EnumerateDeviceExtensionProperties) {
            return it->second.EnumerateDeviceExtensionProperties(physical_device, layer_name, property_count, properties);
        }
    }

    if (property_count) {
        *property_count = 0;
    }
    return VK_SUCCESS;
}

PPFG_EXPORT VKAPI_ATTR PFN_vkVoidFunction VKAPI_CALL vkGetInstanceProcAddr(VkInstance instance, const char* name) {
    if (!name) {
        return nullptr;
    }

    if (std::strcmp(name, "vkGetInstanceProcAddr") == 0) {
        return reinterpret_cast<PFN_vkVoidFunction>(&vkGetInstanceProcAddr);
    }
    if (std::strcmp(name, "vkGetDeviceProcAddr") == 0) {
        return reinterpret_cast<PFN_vkVoidFunction>(&vkGetDeviceProcAddr);
    }

    if (std::strcmp(name, "vkCreateInstance") == 0) {
        return reinterpret_cast<PFN_vkVoidFunction>(&ppfg::layer_create_instance);
    }
    if (std::strcmp(name, "vkDestroyInstance") == 0) {
        return reinterpret_cast<PFN_vkVoidFunction>(&ppfg::layer_destroy_instance);
    }
    if (std::strcmp(name, "vkCreateDevice") == 0) {
        return reinterpret_cast<PFN_vkVoidFunction>(&ppfg::layer_create_device);
    }
    if (std::strcmp(name, "vkDestroyDevice") == 0) {
        return reinterpret_cast<PFN_vkVoidFunction>(&ppfg::layer_destroy_device);
    }
    if (std::strcmp(name, "vkGetDeviceQueue") == 0) {
        return reinterpret_cast<PFN_vkVoidFunction>(&ppfg::layer_get_device_queue);
    }
    if (std::strcmp(name, "vkGetDeviceQueue2") == 0) {
        return reinterpret_cast<PFN_vkVoidFunction>(&ppfg::layer_get_device_queue2);
    }
    if (std::strcmp(name, "vkCreateSwapchainKHR") == 0) {
        return reinterpret_cast<PFN_vkVoidFunction>(&ppfg::layer_create_swapchain_khr);
    }
    if (std::strcmp(name, "vkDestroySwapchainKHR") == 0) {
        return reinterpret_cast<PFN_vkVoidFunction>(&ppfg::layer_destroy_swapchain_khr);
    }
    if (std::strcmp(name, "vkQueuePresentKHR") == 0) {
        return reinterpret_cast<PFN_vkVoidFunction>(&ppfg::layer_queue_present_khr);
    }
    if (std::strcmp(name, "vkEnumerateInstanceLayerProperties") == 0) {
        return reinterpret_cast<PFN_vkVoidFunction>(&vkEnumerateInstanceLayerProperties);
    }
    if (std::strcmp(name, "vkEnumerateInstanceExtensionProperties") == 0) {
        return reinterpret_cast<PFN_vkVoidFunction>(&vkEnumerateInstanceExtensionProperties);
    }
    if (std::strcmp(name, "vkEnumerateDeviceLayerProperties") == 0) {
        return reinterpret_cast<PFN_vkVoidFunction>(&vkEnumerateDeviceLayerProperties);
    }
    if (std::strcmp(name, "vkEnumerateDeviceExtensionProperties") == 0) {
        return reinterpret_cast<PFN_vkVoidFunction>(&vkEnumerateDeviceExtensionProperties);
    }

    return ppfg::get_instance_fallback_proc_addr(instance, name);
}

PPFG_EXPORT VKAPI_ATTR PFN_vkVoidFunction VKAPI_CALL vkGetDeviceProcAddr(VkDevice device, const char* name) {
    if (!name) {
        return nullptr;
    }

    if (std::strcmp(name, "vkGetDeviceProcAddr") == 0) {
        return reinterpret_cast<PFN_vkVoidFunction>(&vkGetDeviceProcAddr);
    }
    if (std::strcmp(name, "vkGetDeviceQueue") == 0) {
        return reinterpret_cast<PFN_vkVoidFunction>(&ppfg::layer_get_device_queue);
    }
    if (std::strcmp(name, "vkGetDeviceQueue2") == 0) {
        return reinterpret_cast<PFN_vkVoidFunction>(&ppfg::layer_get_device_queue2);
    }
    if (std::strcmp(name, "vkCreateSwapchainKHR") == 0) {
        return reinterpret_cast<PFN_vkVoidFunction>(&ppfg::layer_create_swapchain_khr);
    }
    if (std::strcmp(name, "vkDestroySwapchainKHR") == 0) {
        return reinterpret_cast<PFN_vkVoidFunction>(&ppfg::layer_destroy_swapchain_khr);
    }
    if (std::strcmp(name, "vkQueuePresentKHR") == 0) {
        return reinterpret_cast<PFN_vkVoidFunction>(&ppfg::layer_queue_present_khr);
    }
    if (std::strcmp(name, "vkDestroyDevice") == 0) {
        return reinterpret_cast<PFN_vkVoidFunction>(&ppfg::layer_destroy_device);
    }

    return ppfg::get_device_fallback_proc_addr(device, name);
}

PPFG_EXPORT VKAPI_ATTR VkResult VKAPI_CALL vkNegotiateLoaderLayerInterfaceVersion(VkNegotiateLayerInterface* version_struct) {
    if (!version_struct || version_struct->sType != LAYER_NEGOTIATE_INTERFACE_STRUCT) {
        return VK_ERROR_INITIALIZATION_FAILED;
    }
    if (version_struct->loaderLayerInterfaceVersion < 2) {
        return VK_ERROR_INITIALIZATION_FAILED;
    }

    version_struct->loaderLayerInterfaceVersion = 2;
    version_struct->pfnGetInstanceProcAddr = vkGetInstanceProcAddr;
    version_struct->pfnGetDeviceProcAddr = vkGetDeviceProcAddr;
    version_struct->pfnGetPhysicalDeviceProcAddr = nullptr;

    ppfg::log_info(std::string("vkNegotiateLoaderLayerInterfaceVersion ok; mode=") + ppfg::mode_name(ppfg::current_mode()));
    return VK_SUCCESS;
}

} // extern "C"
