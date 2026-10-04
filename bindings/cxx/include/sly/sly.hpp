#pragma once

#include <array>
#include <chrono>
#include <cstddef>
#include <cstdint>
#include <iterator>
#include <memory>
#include <optional>
#include <span>
#include <stdexcept>
#include <string>
#include <vector>

namespace sly {
std::string settings_api_key();
void settings_set_api_key(const std::string& key);
std::string settings_frontend(const std::string& name);
void settings_save_frontend(const std::string& name, const std::string& json);

class Error final : public std::runtime_error {
public:
    using std::runtime_error::runtime_error;
};

enum class Button : std::uint8_t {
    Up = 0,
    Down = 1,
    Left = 2,
    Right = 3,
    A = 4,
    B = 5,
    X = 6,
    Y = 7,
    L = 8,
    R = 9,
    Start = 10,
    Select = 11,
};

inline constexpr std::size_t BUTTON_COUNT = 12;
static_assert(
    static_cast<std::size_t>(Button::Select) + 1 == BUTTON_COUNT,
    "BUTTON_COUNT must cover every Button variant"
);

using ButtonState = std::array<bool, BUTTON_COUNT>;

struct AudioSpec {
    std::uint32_t sample_rate{};
    std::uint8_t channels{};
};

enum class TelevisionSystem : std::uint8_t {
    Ntsc = 0,
    Pal = 1
};
enum class BackupKind : std::uint8_t {
    Sram = 1,
    Fram = 2,
    Eeprom = 3,
    Flash = 4
};

struct ControllerInfo {
    std::uint32_t port{};
    std::string name;
    std::vector<Button> buttons;
};

struct CartridgeInfo {
    std::string title;
    std::string system;
    std::string region_name;
    std::uint32_t rom_crc32{};
    bool backup_battery_backed{};
    std::size_t backup_capacity{};
    std::string rom_sha1;
    std::size_t rom_size{};
    std::optional<TelevisionSystem> television_system;
    std::optional<BackupKind> backup_kind;
};

// The pixel span is borrowed from the core and remains valid only until the
// next non-const operation on that core.
struct FrameView {
    std::uint32_t width{};
    std::uint32_t height{};
    std::span<const std::uint8_t> rgba{};

    [[nodiscard]] explicit operator bool() const noexcept { return !rgba.empty(); }
};

struct Frame {
    std::uint32_t width{};
    std::uint32_t height{};
    std::vector<std::uint8_t> rgba;

    [[nodiscard]] explicit operator bool() const noexcept { return !rgba.empty(); }
};

class SystemCatalog;
class Core;

class SystemDescriptor final {
public:
    [[nodiscard]] std::size_t index() const noexcept { return index_; }

    [[nodiscard]] std::string slug() const;
    [[nodiscard]] std::string name() const;
    [[nodiscard]] std::string qualified_name() const;
    [[nodiscard]] std::vector<std::string> extensions() const;
    [[nodiscard]] std::string rom_sha1(std::span<const std::uint8_t> rom) const;
    [[nodiscard]] CartridgeInfo inspect(std::span<const std::uint8_t> rom) const;
    [[nodiscard]] std::vector<std::uint8_t>
    fetch_wheel_artwork(std::span<const std::uint8_t> rom) const;
    [[nodiscard]] Core create(std::span<const std::uint8_t> rom) const;

    friend bool operator==(SystemDescriptor, SystemDescriptor) = default;

private:
    explicit SystemDescriptor(std::size_t index) noexcept : index_(index) {}

    std::size_t index_{};

    friend class SystemCatalog;
};

class SystemCatalog final {
public:
    class iterator final {
    public:
        using iterator_concept = std::forward_iterator_tag;
        using iterator_category = std::forward_iterator_tag;
        using value_type = SystemDescriptor;
        using difference_type = std::ptrdiff_t;

        iterator() = default;

        [[nodiscard]] value_type operator*() const noexcept;
        iterator& operator++() noexcept;
        iterator operator++(int) noexcept;

        friend bool operator==(iterator, iterator) = default;

    private:
        explicit iterator(std::size_t index) noexcept : index_(index) {}

        std::size_t index_{};

        friend class SystemCatalog;
    };

    [[nodiscard]] std::size_t size() const noexcept;

    [[nodiscard]] bool empty() const noexcept { return size() == 0; }

    [[nodiscard]] SystemDescriptor at(std::size_t index) const;
    [[nodiscard]] SystemDescriptor operator[](std::size_t index) const noexcept;

    [[nodiscard]] iterator begin() const noexcept { return iterator(0); }

    [[nodiscard]] iterator end() const noexcept { return iterator(size()); }
};

[[nodiscard]] SystemCatalog systems() noexcept;

class Core final {
public:
    [[nodiscard]] static Core create(SystemDescriptor system, std::span<const std::uint8_t> rom);

    Core(Core&&) noexcept;
    Core& operator=(Core&&) noexcept;
    ~Core();

    Core(const Core&) = delete;
    Core& operator=(const Core&) = delete;

    void reset() noexcept;
    void step() noexcept;
    void run_frame() noexcept;

    [[nodiscard]] std::chrono::nanoseconds frame_interval() const noexcept;
    void set_button(std::uint32_t player, Button button, bool pressed) noexcept;
    [[nodiscard]] std::vector<ControllerInfo> controllers() const;
    /// Index 0 is the quick slot, 1 to 10 are the numbered slots.
    [[nodiscard]] std::string state_path(std::uint8_t index = 0) const;
    [[nodiscard]] std::string save_ram_path() const;

    [[nodiscard]] std::optional<AudioSpec> audio_spec() const noexcept;

    // The returned samples remain valid only until the next non-const
    // operation on this core.
    [[nodiscard]] std::span<const std::int16_t> drain_audio() noexcept;

    [[nodiscard]] FrameView frame() const noexcept;
    [[nodiscard]] Frame copy_frame() const;

    [[nodiscard]] std::vector<std::uint8_t> save_state() const;
    void load_state(std::span<const std::uint8_t> bytes);

    // The returned bytes remain valid only until the next non-const operation
    // on this core.
    [[nodiscard]] std::span<const std::uint8_t> save_ram() const noexcept;
    [[nodiscard]] bool load_save_ram(std::span<const std::uint8_t> bytes) noexcept;
    [[nodiscard]] std::span<const std::uint8_t> persistable_save_ram() const noexcept;
    void inhibit_save_ram_persistence() const noexcept;

    [[nodiscard]] CartridgeInfo cartridge_info() const;

private:
    struct Impl;

    explicit Core(std::unique_ptr<Impl> impl) noexcept;

    std::unique_ptr<Impl> impl_;
};
} // namespace sly
