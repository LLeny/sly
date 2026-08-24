#include "player_window.h"

#include <QFile>

#include <cstddef>
#include <cstdint>
#include <span>
#include <utility>

namespace {
sly::Core load_core(const sly::SystemDescriptor system, const QString& rom_path) {
    QFile rom(rom_path);
    if (!rom.open(QIODevice::ReadOnly)) {
        throw sly::Error(
            QStringLiteral("Unable to open ROM: %1").arg(rom.errorString()).toStdString()
        );
    }

    const auto bytes = rom.readAll();
    const auto rom_bytes = std::span<const std::uint8_t>(
        reinterpret_cast<const std::uint8_t*>(bytes.constData()),
        static_cast<std::size_t>(bytes.size())
    );
    return system.create(rom_bytes);
}
} // namespace

PlayerWindow::PlayerWindow(
    const sly::SystemDescriptor system, QString rom_path, QString title, QWidget* parent
)
    : QMainWindow(parent), system_(system), rom_path_(std::move(rom_path)),
      title_(std::move(title)), core_(load_core(system_, rom_path_)) {
    setWindowTitle(title_);

    const auto frame = core_.frame();
    setMinimumSize(static_cast<int>(frame.width), static_cast<int>(frame.height));
}
