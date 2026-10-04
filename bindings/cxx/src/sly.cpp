#include "sly/sly.hpp"

#include "sly-cxx/src/lib.rs.h"

#include <exception>
#include <ranges>
#include <utility>

namespace sly {
namespace {
std::string to_string(const rust::String& value) {
    return {value.data(), value.size()};
}

template <typename T> std::vector<T> to_vector(const rust::Vec<T>& value) {
    if (value.empty()) {
        return {};
    }
    return {value.data(), value.data() + value.size()};
}

std::vector<std::string> to_strings(const rust::Vec<rust::String>& values) {
    std::vector<std::string> result;
    result.reserve(values.size());
    for (const auto& value : values) {
        result.push_back(to_string(value));
    }
    return result;
}

template <typename T> rust::Slice<const T> to_slice(std::span<const T> value) noexcept {
    return {value.data(), value.size()};
}

ffi::System raw_system(const SystemDescriptor system) {
    try {
        return ffi::system_at(system.index());
    } catch (const std::exception& error) {
        throw Error(error.what());
    }
}

CartridgeInfo to_cartridge_info(ffi::CartridgeInfo value) {
    return {
        .title = to_string(value.title),
        .system = to_string(value.system),
        .region_name = to_string(value.region_name),
        .rom_crc32 = value.rom_crc32,
        .backup_battery_backed = value.backup_battery_backed,
        .backup_capacity = value.backup_capacity,
        .rom_sha1 = to_string(value.rom_sha1),
        .rom_size = value.rom_size,
        .television_system = value.tv_system < 2
                                 ? std::optional(static_cast<TelevisionSystem>(value.tv_system))
                                 : std::nullopt,
        .backup_kind = value.backup_kind > 0
                           ? std::optional(static_cast<BackupKind>(value.backup_kind))
                           : std::nullopt,
    };
}

ffi::Button raw_button(const Button button) noexcept {
    switch (button) {
        case Button::Up:
            return ffi::Button::Up;
        case Button::Down:
            return ffi::Button::Down;
        case Button::Left:
            return ffi::Button::Left;
        case Button::Right:
            return ffi::Button::Right;
        case Button::A:
            return ffi::Button::A;
        case Button::B:
            return ffi::Button::B;
        case Button::X:
            return ffi::Button::X;
        case Button::Y:
            return ffi::Button::Y;
        case Button::L:
            return ffi::Button::L;
        case Button::R:
            return ffi::Button::R;
        case Button::Start:
            return ffi::Button::Start;
        case Button::Select:
            return ffi::Button::Select;
    }
    return ffi::Button::Up;
}
} // namespace

static_assert(std::ranges::forward_range<SystemCatalog>);

std::string SystemDescriptor::slug() const {
    try {
        return to_string(ffi::system_slug(raw_system(*this)));
    } catch (const Error&) {
        throw;
    } catch (const std::exception& error) {
        throw Error(error.what());
    }
}

std::string SystemDescriptor::name() const {
    try {
        return to_string(ffi::system_name(raw_system(*this)));
    } catch (const Error&) {
        throw;
    } catch (const std::exception& error) {
        throw Error(error.what());
    }
}

std::string SystemDescriptor::qualified_name() const {
    try {
        return to_string(ffi::system_qualified_name(raw_system(*this)));
    } catch (const Error&) {
        throw;
    } catch (const std::exception& error) {
        throw Error(error.what());
    }
}

std::vector<std::string> SystemDescriptor::extensions() const {
    try {
        return to_strings(ffi::system_extensions(raw_system(*this)));
    } catch (const Error&) {
        throw;
    } catch (const std::exception& error) {
        throw Error(error.what());
    }
}

std::string SystemDescriptor::rom_sha1(const std::span<const std::uint8_t> rom) const {
    try {
        return to_string(ffi::rom_sha1(raw_system(*this), to_slice(rom)));
    } catch (const Error&) {
        throw;
    } catch (const std::exception& error) {
        throw Error(error.what());
    }
}

CartridgeInfo SystemDescriptor::inspect(const std::span<const std::uint8_t> rom) const {
    try {
        return to_cartridge_info(ffi::inspect_cartridge(raw_system(*this), to_slice(rom)));
    } catch (const Error&) {
        throw;
    } catch (const std::exception& error) {
        throw Error(error.what());
    }
}

std::vector<std::uint8_t>
SystemDescriptor::fetch_wheel_artwork(const std::span<const std::uint8_t> rom) const {
    try {
        return to_vector(ffi::fetch_wheel_artwork(raw_system(*this), to_slice(rom)));
    } catch (const Error&) {
        throw;
    } catch (const std::exception& error) {
        throw Error(error.what());
    }
}

SystemCatalog::iterator::value_type SystemCatalog::iterator::operator*() const noexcept {
    return SystemDescriptor(index_);
}

SystemCatalog::iterator& SystemCatalog::iterator::operator++() noexcept {
    ++index_;
    return *this;
}

SystemCatalog::iterator SystemCatalog::iterator::operator++(int) noexcept {
    auto previous = *this;
    ++*this;
    return previous;
}

std::size_t SystemCatalog::size() const noexcept {
    return ffi::system_count();
}

SystemDescriptor SystemCatalog::at(const std::size_t index) const {
    try {
        static_cast<void>(ffi::system_at(index));
        return SystemDescriptor(index);
    } catch (const std::exception& error) {
        throw Error(error.what());
    }
}

SystemDescriptor SystemCatalog::operator[](const std::size_t index) const noexcept {
    return SystemDescriptor(index);
}

SystemCatalog systems() noexcept {
    return {};
}

struct Core::Impl {
    explicit Impl(rust::Box<ffi::SlyCore> core) : core(std::move(core)) {}

    rust::Box<ffi::SlyCore> core;
};

Core Core::create(const SystemDescriptor system, const std::span<const std::uint8_t> rom) {
    try {
        return Core(std::make_unique<Impl>(ffi::create_core(raw_system(system), to_slice(rom))));
    } catch (const Error&) {
        throw;
    } catch (const std::exception& error) {
        throw Error(error.what());
    }
}

Core::Core(std::unique_ptr<Impl> impl) noexcept : impl_(std::move(impl)) {}

Core::Core(Core&&) noexcept = default;

Core& Core::operator=(Core&&) noexcept = default;

Core::~Core() = default;

Core SystemDescriptor::create(const std::span<const std::uint8_t> rom) const {
    return Core::create(*this, rom);
}

void Core::reset() noexcept {
    impl_->core->reset();
}

void Core::step() noexcept {
    impl_->core->step();
}

void Core::run_frame() noexcept {
    impl_->core->run_frame();
}

std::chrono::nanoseconds Core::frame_interval() const noexcept {
    return std::chrono::nanoseconds(impl_->core->frame_interval_ns());
}

void Core::set_button(
    const std::uint32_t player, const Button button, const bool pressed
) noexcept {
    impl_->core->set_button(player, raw_button(button), pressed);
}

std::optional<AudioSpec> Core::audio_spec() const noexcept {
    const auto spec = impl_->core->audio_spec();
    if (spec.channels == 0) {
        return std::nullopt;
    }
    return AudioSpec{.sample_rate = spec.sample_rate, .channels = spec.channels};
}

std::vector<ControllerInfo> Core::controllers() const {
    std::vector<ControllerInfo> result;
    for (const auto& controller : impl_->core->controllers()) {
        ControllerInfo info{
            .port = controller.port, .name = to_string(controller.name), .buttons = {}
        };
        for (const auto button : controller.buttons) {
            info.buttons.push_back(static_cast<Button>(button));
        }
        result.push_back(std::move(info));
    }
    return result;
}

std::span<const std::int16_t> Core::drain_audio() noexcept {
    const auto samples = impl_->core->drain_audio();
    return {samples.data(), samples.size()};
}

FrameView Core::frame() const noexcept {
    const auto info = impl_->core->frame_info();
    const auto pixels = impl_->core->frame_rgba();
    return {
        .width = info.width,
        .height = info.height,
        .rgba = {pixels.data(), pixels.size()},
    };
}

Frame Core::copy_frame() const {
    const auto info = impl_->core->frame_info();
    if (info.width == 0 || info.height == 0) {
        return {};
    }

    Frame frame{
        .width = info.width,
        .height = info.height,
        .rgba = std::vector<std::uint8_t>(
            static_cast<std::size_t>(info.width) * static_cast<std::size_t>(info.height) * 4
        ),
    };
    if (!impl_->core->copy_frame_rgba({frame.rgba.data(), frame.rgba.size()})) {
        return {};
    }
    return frame;
}

std::vector<std::uint8_t> Core::save_state() const {
    try {
        return to_vector(impl_->core->save_state());
    } catch (const std::exception& error) {
        throw Error(error.what());
    }
}

void Core::load_state(const std::span<const std::uint8_t> bytes) {
    try {
        impl_->core->load_state(to_slice(bytes));
    } catch (const std::exception& error) {
        throw Error(error.what());
    }
}

std::span<const std::uint8_t> Core::save_ram() const noexcept {
    const auto bytes = impl_->core->save_ram();
    return {bytes.data(), bytes.size()};
}

std::string Core::state_path(const std::uint8_t index) const {
    try {
        return to_string(impl_->core->state_path(index));
    } catch (const std::exception& error) {
        throw Error(error.what());
    }
}

std::string Core::save_ram_path() const {
    return to_string(impl_->core->save_ram_path());
}

bool Core::load_save_ram(const std::span<const std::uint8_t> bytes) noexcept {
    return impl_->core->load_save_ram(to_slice(bytes));
}

std::span<const std::uint8_t> Core::persistable_save_ram() const noexcept {
    const auto bytes = impl_->core->persistable_save_ram();
    return {bytes.data(), bytes.size()};
}

void Core::inhibit_save_ram_persistence() const noexcept {
    impl_->core->inhibit_save_ram_persistence();
}

CartridgeInfo Core::cartridge_info() const {
    return to_cartridge_info(impl_->core->cartridge_info());
}

std::string settings_api_key() {
    return to_string(ffi::api_key());
}

void settings_set_api_key(const std::string& key) {
    ffi::set_api_key(key);
}

std::string settings_frontend(const std::string& name) {
    return to_string(ffi::frontend_json(name));
}

void settings_save_frontend(const std::string& name, const std::string& json) {
    ffi::save_frontend_json(name, json);
}

} // namespace sly
