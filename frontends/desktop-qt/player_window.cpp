#include "player_window.h"

#include <QFile>
#include <QMenuBar>

#include <cstdint>
#include <format>
#include <span>
#include <utility>

namespace {
sly::Core load_core(const sly::SystemDescriptor system, const QString& rom_path) {
    QFile rom(rom_path);
    if (!rom.open(QIODevice::ReadOnly)) {
        throw sly::Error(std::format("Unable to open ROM: {}", rom.errorString().toStdString()));
    }

    const auto bytes = rom.readAll();
    const auto rom_bytes =
        std::span(reinterpret_cast<const std::uint8_t*>(bytes.constData()), bytes.size());
    return system.create(rom_bytes);
}
} // namespace

PlayerWindow::PlayerWindow(
    const sly::SystemDescriptor system, QString rom_path, QString title, QWidget* parent
)
    : QMainWindow(parent), system_(system), rom_path_(std::move(rom_path)),
      title_(std::move(title)), core_(load_core(system_, rom_path_)) {
    setWindowTitle(title_);

    video_widget_ = new VideoWidget(this);
    setCentralWidget(video_widget_);

    // QMenuBar* menu_bar = new QMenuBar(this);
    // setMenuBar(menu_bar);

    // auto* video_menu = menu_bar->addMenu(tr("&Video"));
    // auto* action = video_menu->addAction(tr("&Fullscreen"));
    // action->setCheckable(true);
    // action->setChecked(system.index() == 0);

    const auto frame = core_.frame();
    setMinimumSize(static_cast<int>(frame.width), static_cast<int>(frame.height));
    video_widget_->set_frame(frame);

    frame_timer_ = new QChronoTimer(core_.frame_interval(), this);
    frame_timer_->setTimerType(Qt::PreciseTimer);
    connect(frame_timer_, &QChronoTimer::timeout, this, [this] {
        core_.run_frame();
        video_widget_->set_frame(core_.frame());
    });
    frame_timer_->start();
}
