#pragma once

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

struct AudioSpec {
    std::uint32_t sample_rate{};
    std::uint8_t channels{};
};

struct CartridgeInfo {
    std::string title;
    std::string system;
    std::string region_name;
    bool pal{};
    std::uint32_t rom_crc32{};
    bool has_battery{};
    std::size_t sram_size{};
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

    [[nodiscard]] CartridgeInfo cartridge_info() const;

private:
    struct Impl;

    explicit Core(std::unique_ptr<Impl> impl) noexcept;

    std::unique_ptr<Impl> impl_;
};
} // namespace sly
